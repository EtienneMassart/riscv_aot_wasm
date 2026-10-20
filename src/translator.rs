
mod register_allocation;
mod vb_resolution;
mod visit_operator;
mod helper_struct;
mod library_function;

use crate::isa_model::{RegisterPair, ValueType};
use crate::parse_and_translate::GlobalTranslator;
use crate::{
    isa_model::{
        RiscvInstr, GPRegister, FPRegister, Register, Const12,
        MapperLocation, ValueSize, FRAME_POINTER,
        STACK_BASE, STACK_POINTER,
    },
    parse_and_translate::WasmRuntime,
    vb::{AtomicVB, BinaryVB, UnaryVB, VB},
};
use crate::isa_model::machine_instructions::RoundingMode;
use helper_struct::{BlockLabel, BlockResult};
pub(crate) use helper_struct::{CfgJob, StackHeight, LabelIndex, StackState};

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use wasmparser::LocalsReader;

const MAX_LOCAL_REGISTERS: u8 = 20; // we leave registers x20-x27 for temporary use in code generation; they are not allocated to local variables
const MAX_LOCAL_FP_REGISTERS: u8 = 16;


pub(crate) struct Translator<'a, 'b> {
    pub vb_stack: Vec<VB>, // valent blocks
    pub locals_map: Vec<MapperLocation>, // location of local variables
    pub locked_register: Option<Register>,
    pub cfg_label_stack: Vec<BlockLabel>, // stack of control flow labels for if, block, loop...
    pub cfg_block_result_stack: Vec<BlockResult>, // stack of control flow block results
    pub vb_stack_ptr_stack: Vec<usize>, // points to the top of the stack at the beginning of each cfg block to allow reset when seeing br
    pub dead_code_flag_stack: Vec<bool>, // to keep track of nested blocks and set dead code flag for the current block
    pub function_type_index: u32,
    pub global_translator: &'a mut GlobalTranslator,
    pub wasm_runtime: &'a mut WasmRuntime<'b>,
    pub cfg_label_map: Vec<Option<usize>>, // offset of each cfg label in the instruction memory
    pub cfg_jobs: Vec<CfgJob>,
    pub modeled_stack_height: StackHeight,
    pub stack_pointer_height: StackHeight,
    pub local_save_slots: Vec<i16>, // fp-relative save slot for each body local (index 0 = first non-param local)
    pub num_params: usize,
    pub total_frame_offset: i16, // sp = fp + total_frame_offset at STACK_BASE; used to recover fp after calls
}

impl<'a, 'b> Translator<'a, 'b> {
    /// Creates a new Translator instance. Translating a WebAssembly function requires a Translator instance. 
    /// This function generates the machine code of the prologue for the function as well.
    ///
    /// # Arguments
    ///
    /// * `type_index` - The index of the function type in the wasm module.
    /// * `locals_reader` - The locals reader of the function , provided from the wasm Parser
    /// * `global_translator` - The global translator instance.
    /// * `wasm_runtime` - The wasm runtime instance.
    ///
    /// # Returns
    ///  A Translator instance.
    ///
    pub(crate) fn new(
        type_index: u32,
        locals_reader: LocalsReader<'_>,
        global_translator: &'a mut GlobalTranslator,
        wasm_runtime: &'a mut WasmRuntime<'b>,
    ) -> Self {
        wasm_runtime.push_instruction(RiscvInstr::ADDI { // initialize frame pointer with stack pointer
            rd: FRAME_POINTER,
            rs1: STACK_POINTER,
            imm: Const12::new(0),
        });
        
        // Non-leaf safety: save function return address once per function invocation.
        wasm_runtime.push_instruction(RiscvInstr::SW {
            rs2: GPRegister(1),
            rs1: FRAME_POINTER,
            imm: Const12::new(-4),
        });

        let mut locals_map = Vec::new();
        let mut alloc_register_index: u8 = 8; // registers x0-x7 are reserved for specific purposes, so we start allocating from x8
        let mut alloc_fp_register_index: u8 = 0; // every FP register is available for allocation, we can start allocating from f0

        Self::allocate_params(&mut locals_map, wasm_runtime, type_index, &mut alloc_register_index, &mut alloc_fp_register_index);
        let num_params = locals_map.len();


        let mut frame_cursor: i16 = -4; // ra at fp-4; body-local slots allocated from here downward
        let mut local_save_slots: Vec<i16> = Vec::new();
        Self::allocate_locals(&mut locals_map, wasm_runtime, locals_reader, &mut alloc_register_index, &mut alloc_fp_register_index, &mut frame_cursor, &mut local_save_slots);

        let total_frame_offset = frame_cursor; // STACK_BASE = fp + total_frame_offset, so fp = STACK_BASE - total_frame_offset
        wasm_runtime.push_instruction(RiscvInstr::ADDI {
            rd: STACK_POINTER,
            rs1: STACK_POINTER,
            imm: Const12::new(total_frame_offset),
        });
        wasm_runtime.push_instruction(RiscvInstr::ADDI {
            rd: STACK_BASE,
            rs1: STACK_POINTER,
            imm: Const12::new(0),
        });

        Translator {
            vb_stack: vec![],
            locals_map,
            cfg_label_map: vec![],
            cfg_block_result_stack: vec![],
            locked_register: None,
            cfg_label_stack: vec![],
            dead_code_flag_stack: vec![false],
            function_type_index: type_index,
            vb_stack_ptr_stack: vec![0],
            global_translator,
            wasm_runtime,
            cfg_jobs: vec![],
            modeled_stack_height: StackHeight::from(0),
            stack_pointer_height: StackHeight::from(0),
            local_save_slots,
            num_params,
            total_frame_offset,
        }
    }

    /// Allocates registers for the function's parameters. If there are not enough registers, the parameter is kept where it is on the stack.
    /// Parameters are always passed on the stack, so we need to load them into registers if we want to use them in the function.
    /// They are at a positive offset from the frame pointer at function entry, and are loaded from there.
    fn allocate_params(
        locals_map: &mut Vec<MapperLocation>,
        wasm_runtime: &mut WasmRuntime<'_>,
        type_index: u32,
        alloc_register_index: &mut u8,
        alloc_fp_register_index: &mut u8,
    ) {
        let function_type = wasm_runtime.types[type_index as usize].unwrap_func();
        let params_type: Vec<ValueType> = function_type
            .params()
            .iter()
            .map(ValueType::from_wasmparser)
            .collect();

        // params_offset[i] = byte offset of param i relative to fp at function entry (positive = on caller stack)
        let params_offset: Vec<i16> = {
            let mut params_offset = vec![0i16; params_type.len()];
            let mut offset = 0;
            for i in (0..params_type.len()).rev() {
                params_offset[i] = offset;
                offset += params_type[i].as_bytes() as i16;
            }
            params_offset
        };


        // allocate registers for parameters, if there are not enough registers, the parameter is kept where it is on the stack
        for i in 0..params_type.len() {
            match params_type[i] {
                ValueType::I32 => {
                    if *alloc_register_index < MAX_LOCAL_REGISTERS {
                        locals_map.push(MapperLocation::GPRegister(GPRegister(*alloc_register_index)));
                        wasm_runtime.push_instruction(RiscvInstr::LW {
                            rd: GPRegister(*alloc_register_index),
                            rs1: FRAME_POINTER,
                            imm: Const12::new(params_offset[i]),
                        });
                        *alloc_register_index += 1;
                    }

                    else {
                        locals_map.push(MapperLocation::Frame {
                            val_type: ValueType::I32,
                            offset: params_offset[i],
                        });
                    }
                    
                },
                ValueType::I64 => {
                    if *alloc_register_index < MAX_LOCAL_REGISTERS-1 { // we need at least 2 registers for the i64 param
                        locals_map.push(MapperLocation::RegisterPair(RegisterPair::new(
                            *alloc_register_index,
                            *alloc_register_index + 1,
                        )));
                        wasm_runtime.push_instruction(RiscvInstr::LW {
                            rd: GPRegister(*alloc_register_index),
                            rs1: FRAME_POINTER,
                            imm: Const12::new(params_offset[i]),
                        });
                        wasm_runtime.push_instruction(RiscvInstr::LW {
                            rd: GPRegister(*alloc_register_index + 1),
                            rs1: FRAME_POINTER,
                            imm: Const12::new(params_offset[i] + 4),
                        });
                        *alloc_register_index += 2;
                    } else {
                        locals_map.push(MapperLocation::Frame {
                            val_type: ValueType::I64,
                            offset: params_offset[i],
                        });
                    }
                }
                ValueType::F32 => {
                    if *alloc_fp_register_index < MAX_LOCAL_FP_REGISTERS {
                        let register = FPRegister::new(*alloc_fp_register_index);
                        locals_map.push(MapperLocation::FPRegister {
                            reg: register,
                            val_size: ValueSize::Word,
                        });
                        wasm_runtime.push_instruction(RiscvInstr::FLW {
                            rd: register,
                            rs1: FRAME_POINTER,
                            imm: Const12::new(params_offset[i]),
                        });
                        *alloc_fp_register_index += 1;
                    } else {
                        locals_map.push(MapperLocation::Frame {
                            val_type: ValueType::F32,
                            offset: params_offset[i],
                        });
                    }
                }
                ValueType::F64 => {
                    if *alloc_fp_register_index < MAX_LOCAL_FP_REGISTERS {
                        let register = FPRegister::new(*alloc_fp_register_index);
                        locals_map.push(MapperLocation::FPRegister {
                            reg: register,
                            val_size: ValueSize::DoubleWord,
                        });
                        wasm_runtime.push_instruction(RiscvInstr::FLD {
                            rd: register,
                            rs1: FRAME_POINTER,
                            imm: Const12::new(params_offset[i]),
                        });
                        *alloc_fp_register_index += 1;
                    } else {
                        locals_map.push(MapperLocation::Frame {
                            val_type: ValueType::F64,
                            offset: params_offset[i],
                        });
                    }
                }
            }
        }
    }

    /// Allocate memory and registers for the function's local variables.
    /// Each local variable is given a MapperLocation, which can be a register or a frame slot.
    /// If there are enough registers, the local variable is allocated to a register. Otherwise, it is allocated to a frame slot. In both case it will be initialized to 0.
    /// However, even if a local variable is allocated to a register, a frame slot is still reserved for it. It will be its backup location when the register needs to be spilled.
    /// That reserved frame slot is not initialized to 0, it is just a backup location.
    /// These frame slots, whether allocated or reserved, are allocated from the frame_cursor downwards, starting from fp-4 (fp-0 is reserved for the return address).
    fn allocate_locals(
        locals_map: &mut Vec<MapperLocation>,
        wasm_runtime: &mut WasmRuntime<'_>,
        locals_reader: LocalsReader<'_>,
        alloc_register_index: &mut u8,
        alloc_fp_register_index: &mut u8,
        frame_cursor: &mut i16,
        local_save_slots: &mut Vec<i16>,
    ) {
        for (count, val_type) in locals_reader.into_iter().map(Result::unwrap) {
            for _ in 0..count {
                let vt = ValueType::from_wasmparser(&val_type);
                *frame_cursor -= vt.as_bytes() as i16;
                let slot = *frame_cursor;
                match vt {
                    ValueType::I32 => {
                        if *alloc_register_index < MAX_LOCAL_REGISTERS {
                            locals_map.push(MapperLocation::GPRegister(GPRegister(*alloc_register_index)));
                            wasm_runtime.push_instruction(RiscvInstr::ADDI { // initialize the register to 0
                                rd: GPRegister(*alloc_register_index),
                                rs1: GPRegister(0),
                                imm: Const12::new(0),
                            });
                            *alloc_register_index += 1;
                        } else {
                            locals_map.push(MapperLocation::Frame { val_type: ValueType::I32, offset: slot });
                            wasm_runtime.push_instruction(RiscvInstr::SW { // initialize the frame slot to 0
                                rs1: FRAME_POINTER,
                                rs2: GPRegister(0),
                                imm: Const12::new(slot),
                            });
                        }
                        local_save_slots.push(slot);
                    }
                    ValueType::I64 => {
                        if *alloc_register_index < MAX_LOCAL_REGISTERS - 1 {
                            locals_map.push(MapperLocation::RegisterPair(RegisterPair::new(*alloc_register_index, *alloc_register_index + 1)));
                            wasm_runtime.push_instruction(RiscvInstr::ADDI {
                                rd: GPRegister(*alloc_register_index),
                                rs1: GPRegister(0),
                                imm: Const12::new(0),
                            });
                            wasm_runtime.push_instruction(RiscvInstr::ADDI {
                                rd: GPRegister(*alloc_register_index + 1),
                                rs1: GPRegister(0),
                                imm: Const12::new(0),
                            });
                            *alloc_register_index += 2;
                        } else {
                            locals_map.push(MapperLocation::Frame { val_type: ValueType::I64, offset: slot });
                            wasm_runtime.push_instruction(RiscvInstr::SW {
                                rs1: FRAME_POINTER,
                                rs2: GPRegister(0),
                                imm: Const12::new(slot),
                            });
                            wasm_runtime.push_instruction(RiscvInstr::SW {
                                rs1: FRAME_POINTER,
                                rs2: GPRegister(0),
                                imm: Const12::new(slot + 4),
                            });
                        }
                        local_save_slots.push(slot);
                    }
                    ValueType::F32 => {
                        if *alloc_fp_register_index < MAX_LOCAL_FP_REGISTERS {
                            let register = FPRegister::new(*alloc_fp_register_index);
                            locals_map.push(MapperLocation::FPRegister { reg: register, val_size: ValueSize::Word });
                            wasm_runtime.push_instruction(RiscvInstr::FCVTSW {
                                rd: register,
                                rs1: GPRegister(0),
                                rm: RoundingMode::RNE,
                            });
                            *alloc_fp_register_index += 1;
                        } else {
                            locals_map.push(MapperLocation::Frame { val_type: ValueType::F32, offset: slot });
                            wasm_runtime.push_instruction(RiscvInstr::SW {
                                rs1: FRAME_POINTER,
                                rs2: GPRegister(0),
                                imm: Const12::new(slot),
                            });
                        }
                        local_save_slots.push(slot);
                    }
                    ValueType::F64 => {
                        if *alloc_fp_register_index < MAX_LOCAL_FP_REGISTERS {
                            let register = FPRegister::new(*alloc_fp_register_index);
                            locals_map.push(MapperLocation::FPRegister { reg: register, val_size: ValueSize::DoubleWord });
                            wasm_runtime.push_instruction(RiscvInstr::FCVTDW {
                                rd: register,
                                rs1: GPRegister(0),
                                rm: RoundingMode::RNE,
                            });
                            *alloc_fp_register_index += 1;
                        } else {
                            locals_map.push(MapperLocation::Frame { val_type: ValueType::F64, offset: slot });
                            wasm_runtime.push_instruction(RiscvInstr::SW {
                                rs1: FRAME_POINTER,
                                rs2: GPRegister(0),
                                imm: Const12::new(slot),
                            });
                            wasm_runtime.push_instruction(RiscvInstr::SW {
                                rs1: FRAME_POINTER,
                                rs2: GPRegister(0),
                                imm: Const12::new(slot + 4),
                            });
                        }
                        local_save_slots.push(slot);
                    }
                }
            }
        }
    }




    pub(crate) fn stack_logical_offset_to_imm(&mut self, logical_offset: StackHeight) -> i16 {
        self.rebase_stack_pointer_toward(logical_offset);
        let imm = self.stack_pointer_height.to_i32() - logical_offset.to_i32();
        debug_assert!(
            (-2048..=2047).contains(&imm),
            "stack immediate out of 12-bit signed range: logical_offset={}, sp_height={}, imm={}",
            logical_offset.to_usize(),
            self.stack_pointer_height.to_usize(),
            imm
        );
        imm as i16
    }

    pub(crate) fn maybe_rebase_stack_pointer(&mut self) {
        self.rebase_stack_pointer_toward(self.modeled_stack_height);
    }

    fn rebase_stack_pointer_toward(&mut self, logical_offset: StackHeight) {
        while self.stack_pointer_height.to_i32() - logical_offset.to_i32() < -1024 {
            self.push_instruction(RiscvInstr::ADDI {
                rd: STACK_POINTER,
                rs1: STACK_POINTER,
                imm: Const12::new(-1024),
            });
            self.stack_pointer_height = self.stack_pointer_height.add(1024);
        }
        while self.stack_pointer_height.to_i32() - logical_offset.to_i32() > 1024
            && self.stack_pointer_height.to_usize() >= 1024
        {
            self.push_instruction(RiscvInstr::ADDI {
                rd: STACK_POINTER,
                rs1: STACK_POINTER,
                imm: Const12::new(1024),
            });
            self.stack_pointer_height = self.stack_pointer_height.sub(1024);
        }
    }

    fn stack_bytes_for_vb(vb: &VB) -> usize {
        match vb {
            VB::AtomicVB(AtomicVB::Resolved { val_type, .. }) => val_type.as_bytes(),
            _ => 0,
        }
    }

    pub(crate) fn push_vb_stack(&mut self, vb: VB) {
        self.modeled_stack_height = self.modeled_stack_height.add(Self::stack_bytes_for_vb(&vb));
        self.maybe_rebase_stack_pointer();
        self.vb_stack.push(vb);
    }

    pub(crate) fn pop_vb_stack(&mut self) -> VB {
        let vb = self.vb_stack.pop().unwrap();
        let bytes = Self::stack_bytes_for_vb(&vb);
        if bytes != 0 {
            self.modeled_stack_height = self.modeled_stack_height.sub(bytes);
            self.maybe_rebase_stack_pointer();
        }
        vb
    }

    /// Translates a machine instruction to binary format and adds it to the instruction array.
    /// This path auto-compresses when legal.
    pub(crate) fn push_instruction(&mut self, instr: RiscvInstr) {
        self.wasm_runtime.push_instruction(instr);
    }

    /// Translates a machine instruction to binary format and adds it to the instruction array exactly as written.
    /// Use this for instructions after a jump/branch with a specific offset based on the size of the instructions.
    pub(crate) fn push_instruction_no_compress(&mut self, instr: RiscvInstr) {
        self.wasm_runtime.push_instruction_no_compress(instr);
    }

    pub(crate) fn add_atomic_vb(&mut self, new_vb: AtomicVB) {
        let dead_code_flag = *self.dead_code_flag_stack.last().unwrap_or(&false);
        if dead_code_flag {
            return;
        }
        self.push_vb_stack(VB::AtomicVB(new_vb));
    }

    pub(crate) fn add_unary_vb(&mut self, new_vb: UnaryVB) {
        let dead_code_flag = *self.dead_code_flag_stack.last().unwrap_or(&false);
        if dead_code_flag {
            return;
        }
        let child = self.vb_stack.pop().unwrap();
        self.push_vb_stack(VB::UnaryVB {
            vb: new_vb,
            child: Box::new(child),
        });
    }

    pub(crate) fn add_binary_vb(&mut self, new_vb: BinaryVB) {
        let dead_code_flag = *self.dead_code_flag_stack.last().unwrap_or(&false);
        if dead_code_flag {
            return;
        }
        let rhs = self.vb_stack.pop().unwrap();
        let lhs = self.vb_stack.pop().unwrap();
        self.push_vb_stack(VB::BinaryVB {
            vb: new_vb,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        });
    }
}
