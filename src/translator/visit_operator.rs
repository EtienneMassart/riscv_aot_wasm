#![forbid(unsafe_code)]

//! # WebAssembly Instruction Translator
//!
//! This module translates WebAssembly instructions into target machine code for the Aurix processor.
//! It implements the `VisitOperator` trait to handle each WebAssembly instruction type.
//!
//! ## Key Concepts
//!
//! - **VB (Valent Block)**: An IR to represent expressions of values in the WebAssembly operand stack
//! - **VB Stack**: Virtualizes the operand stack at compile-time during translation to machine code
//! - **Dead Code**: Code that will never be executed (e.g., after unreachable or return)
//! - **Label Management**: Tracks jump targets for control flow (blocks, loops, branches)
//! - **Runtime Stack**: The actual stack used during execution (different from VB stack)
//!
//! ## Glossary
//!
//! - **AtomicVB**: A VB representing a single value expression (constant, local, global)
//! - **BinaryVB**: A VB representing an operation expression on two operands
//! - **UnaryVB**: A VB representing an operation expression on one operand
//! - **VB Stack**: Compile-time virtualization of the WebAssembly operand stack
//! - **CFG**: Control Flow Graph - represents the structure of jumps and branches
//! - **MemArg**: Memory argument containing offset and alignment for memory operations
//! - **BlockType**: Describes the signature of a WebAssembly block (empty, single type, or function type)
//! - **MapperLocation**: Specifies where a value should be placed (register, stack, memory)
//!
//! ## Architecture
//!
//! The translator maintains several pieces of state:
//! - `vb_stack`: Virtual representation of the WebAssembly operand stack using VB expressions
//! - `cfg_label_map`: Maps label indices to instruction positions
//! - `dead_code_flag_stack`: Tracks dead code regions in nested blocks
//!
//! ## Translation Process
//!
//! 1. Each `visit_*` method handles one WebAssembly instruction
//! 2. VBs are created to represent value expressions on the operand stack
//! 3. Helper functions centralize common patterns (dead code checks, branching, etc.)
//! 4. VBs are resolved to actual machine instructions when needed
//!
/// This module contains methods that match each wasm instruction to the expected behavior. 
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use wasmparser::{BlockType, BrTable, Ieee32, Ieee64, MemArg, ValType, VisitOperator};


use crate::isa_model::{
    Const12, Const20, GLOBAL_BASE, Immediate, Memsize, STACK_POINTER, FRAME_POINTER, LINEAR_MEMORY_BASE, STACK_BASE,
};
#[cfg(feature = "address-masking")]
use crate::isa_model::ADDRESS_MASK;
use crate::parse_and_translate::WasmRuntime;
use crate::vb::{Address, AtomicVB, BinaryVB, UnaryVB, VB};
use crate::translator::{BlockLabel, BlockResult, CfgJob, LabelIndex, Register, StackHeight, StackState, Translator};

use crate::isa_model::{ TABLE_BASE, machine_instructions::{RiscvInstr, ShAmt5}, ValueSize, MapperLocation, GPRegister, RegisterPair, ValueType, FPRegister};

/// Patches a previously emitted 32-bit JAL at half-word index `hw_index` with a J-type displacement.
fn patch_jal_displacement(runtime: &mut WasmRuntime<'_>, hw_index: usize, disp_bytes: i32) {
    assert_eq!(disp_bytes & 1, 0, "unaligned JAL displacement");
    assert!(
        (-1_048_576..=1_048_574).contains(&disp_bytes),
        "JAL displacement out of ±1 MiB range"
    );
    let low  = runtime.instructions[hw_index] as u32;
    let high = runtime.instructions[hw_index + 1] as u32;
    let mut word = low | (high << 16);
    let imm = (disp_bytes as u32) & 0x001f_ffff;
    let imm20     = (imm >> 20) & 0x1;
    let imm10_1   = (imm >> 1)  & 0x3ff;
    let imm11     = (imm >> 11) & 0x1;
    let imm19_12  = (imm >> 12) & 0xff;
    let mask = (1u32 << 31) | (0x3ffu32 << 21) | (1u32 << 20) | (0xffu32 << 12);
    word &= !mask;
    word |= (imm20 << 31) | (imm10_1 << 21) | (imm11 << 20) | (imm19_12 << 12);
    runtime.instructions[hw_index]     = (word & 0xffff) as u16;
    runtime.instructions[hw_index + 1] = ((word >> 16) & 0xffff) as u16;
}

fn add_i32_imm(
    translator: &mut Translator<'_, '_>,
    rd: GPRegister,
    rs1: GPRegister,
    imm: i32,
    scratch: GPRegister,
) {
    if (-2048..=2047).contains(&imm) {
        translator.push_instruction(RiscvInstr::ADDI {
            rd,
            rs1,
            imm: Const12::new(imm as i16),
        });
        return;
    }

    let upper = (imm.wrapping_add(0x800)) >> 12;
    let lower = (imm & 0xFFF) as i16;
    translator.push_instruction(RiscvInstr::LUI {
        rd: scratch,
        imm: Const20::new(upper),
    });
    if lower != 0 {
        translator.push_instruction(RiscvInstr::ADDI {
            rd: scratch,
            rs1: scratch,
            imm: Const12::new(lower),
        });
    }
    translator.push_instruction(RiscvInstr::ADD {
        rd,
        rs1,
        rs2: scratch,
    });
}

/// macro helps implementing the OperatorVisitor trait. The MVP instructions are left to be implemented manually, while the others default to a panic.
macro_rules! _visit_only_mvp {
    // delegate the macro invocation to sub-invocations of this macro to
    // deal with each instruction on a case-by-case basis.
    ($( @$proposal:ident $op:ident $({ $($arg:ident: $argty:ty),* })? => $visit:ident ($($ann:tt)*))*) => {
        $(
            _visit_only_mvp!(visit_one @$proposal $op $({ $($arg: $argty),* })? => $visit);
        )*
    };

    // MVP instructions are defined manually, so do nothing.
    (visit_one @mvp $($rest:tt)*) => {};

    // a Non-MVP instruction will cause a panic  
    (visit_one @$proposal:ident $op:ident $({ $($arg:ident: $argty:ty),* })? => $visit:ident) => {
        fn $visit(&mut self $($(,$arg: $argty)*)?) -> Self::Output {
            panic!("Operator {:?} is not part of the MVP proposal", stringify!($op));
        }
    }

}

#[derive(Debug)]
enum BlockStyle {
    Block,
    Loop
}

impl Translator<'_, '_> {

    // ================================================================================
    // BLOCK AND CONTROL FLOW MANAGEMENT
    // ================================================================================

    /// Helper method to be called while entering a new block.
    /// 
    /// This method handles the complex state management required when entering WebAssembly
    /// blocks, loops, and if statements. It manages:
    /// - Dead code propagation through nested blocks
    /// - Stack offset calculation for proper cleanup
    /// - Label management for forward/backward jumps
    /// - VB stack state preservation
    fn enter_block(&mut self, blockty: BlockType, block_style: BlockStyle) {

        // Propagate dead code state: if we're already in dead code, the entire block is dead
        let dead_code_flag = *self.dead_code_flag_stack.last().unwrap_or(&false);
        self.dead_code_flag_stack.push(dead_code_flag);

        // Skip all processing if we're in dead code - no instructions will be generated
        if dead_code_flag {
            return;
        }
        
        // Determine the result type and size for this block
        // Empty blocks have no result, Type blocks have a single result,
        // FuncType blocks use the function signature's first result
        // TODO: The fact that block types are always a single valtype (or a function with no argument and one output)
        // is specific to 1.0, might not be checked by wasmparser
        let blockty_value_type = match blockty {
            BlockType::Empty => None,
            BlockType::Type(ty) => Some(ValueType::from_wasmparser(&ty)),
            BlockType::FuncType(type_index) => {
                let funtype = self.wasm_runtime.types[type_index as usize].unwrap_func();
                 funtype.results().get(0).map(|ty| ValueType::from_wasmparser(ty))
            }
        };
        
        // If the block produces a result, we need to resolve all pending VBs now
        // to ensure proper stack ordering when the result expression is eventually produced
        if blockty_value_type.is_some(){
            self.resolve_all();
        }

        // Calculate the runtime stack offset at the start of this block
        // This is needed for proper stack cleanup when the block ends
        let runtime_stack_offset = self.get_current_stack_offset();

        // Convert result size to bytes for stack offset calculations
        let blockty_value_byte_size = blockty_value_type.map(|ty| ty.as_bytes()).unwrap_or(0);
    
        // Calculate stack offsets for two scenarios:
        // 1. end_state: Stack state when reaching the end of the block naturally
        // 2. label_state: Stack state when branching to this block's label position
        //
        // For regular blocks: both scenarios have the same stack state (original + result)
        // For loops: label_state points to loop start (no result), end_state to loop end (with result)
        let block_result = BlockResult {
            // When block ends naturally: stack = original position + result size
            end_state: StackState { height: runtime_stack_offset.add(blockty_value_byte_size), result_type: blockty_value_type },
            label_state: match block_style {
                // Regular block: branch target same as block end
                BlockStyle::Block => StackState { height: runtime_stack_offset.add(blockty_value_byte_size), result_type: blockty_value_type },
                // Loop: branch target is loop start (no accumulated result)
                BlockStyle::Loop => StackState { height: runtime_stack_offset, result_type: None },
            }
        };

        // Track block results for nested blocks using a stack
        self.cfg_block_result_stack.push(block_result);

        // LABEL MANAGEMENT:
        // We maintain a label map (cfg_label_map) that maps label indices to instruction positions.
        // Each nested block gets a label index stored in cfg_label_stack.
        //
        // Two types of jumps:
        // - Backward jumps (loops): target is known immediately (current position)
        // - Forward jumps (blocks): target unknown, use placeholder until block end
        
        // Get next available label index and push to label stack
        self.cfg_label_stack.push(BlockLabel::BlockLoop(self.get_current_label_index()));

        // Set label address based on block style
        match block_style {
            // Block: forward jump target unknown, use placeholder
            BlockStyle::Block => self.add_placeholder_label(),
            // Loop: backward jump target known, use current position
            BlockStyle::Loop => self.add_current_position_label(),
        };

        // Preserve VB stack state at block entry for proper cleanup
        // Each VB represents a value expression on the operand stack
        self.vb_stack_ptr_stack.push(self.vb_stack.len());
        
    }

    /// Helper function that resolves the block's result and adjusts the stack pointer.
    /// 
    /// This handles the complex stack management when a block completes:
    /// - If the block has a result, it's moved to the correct stack position
    /// - Stack pointer is adjusted to clean up the block's local stack space
    /// 
    /// Example: Block with i32 result, original stack offset 16, current offset 24
    /// - Result is in some scratch register
    /// - Need to write result to stack\[16\] and set SP to point to stack\[20\]
    fn resolve_block_result(&mut self, target : StackState ) {
        let StackState { height: offset, result_type: val_type } = target;

        if let Some(val_type) = val_type {
            let stack_location = MapperLocation::Stack { offset: offset.to_i32(), val_type };
            let _ = self.resolve_with_target(Some(&stack_location));
        }
        self.modeled_stack_height = offset;
        self.sync_stack_pointer_to_modeled_height();
    }

    /// resolve the return VB value (if existent) for the wasm function at the end or at a return instruction
    /// result is stored in D[0]/E[0] and then used after function call, then we restore lower context and bitmask goes back to D[0]
    fn resolve_return_value(&mut self) {
        let result_type = self.get_current_function_result_type();
        match result_type {
            Some(ty) => {
                // ABI native return convention: integers in a0/a1, floats in fa0
                let location = match ValueType::from_wasmparser(ty) {
                    ValueType::I32 => MapperLocation::GPRegister(GPRegister(10)),
                    ValueType::I64 => MapperLocation::RegisterPair(RegisterPair::new(10, 11)),
                    ValueType::F32 => MapperLocation::FPRegister{reg: FPRegister(10), val_size: ValueSize::Word},
                    ValueType::F64 => MapperLocation::FPRegister{reg: FPRegister(10), val_size: ValueSize::DoubleWord},
                };
                let _ = self.resolve_with_target(Some(&location));
            },
            None => ()
        }
    }

    fn emit_function_return_sequence(&mut self) {
        self.resolve_return_value();

        // Restore function return address saved in Translator::new prologue.
        // Use FRAME_POINTER so return works even if SP moved during translation/runtime stack operations.
        self.push_instruction(RiscvInstr::LW {
            rd: GPRegister(1),
            rs1: FRAME_POINTER,
            imm: Const12::new(-4),
        });

        // Restore stack pointer to its entry value for this function.
        self.push_instruction(RiscvInstr::ADDI {
            rd: STACK_POINTER,
            rs1: FRAME_POINTER,
            imm: Const12::new(0),
        });
        self.push_instruction(RiscvInstr::RET);
    }
    
    /// Helper function for emitting machine code that stores a value in the linear memory.
    /// Both the value expression and the address expression (dynamic offset) are available as VBs on the stack
    fn store_value_in_memory(&mut self, memarg: MemArg, mem_size: Memsize, value_type: ValueType) {
        if self.check_dead_code() {
            return;
        }
        
        // If any VB expressions depend on memory, we resolve them first to maintain proper ordering
        if self.vb_stack.iter().any(VB::depends_on_memory) {
            let value_vb = self.pop_vb();
            let dynamic_offset_vb = self.pop_vb();
            self.resolve_all();
            self.push_vb_stack(dynamic_offset_vb);
            self.push_vb_stack(value_vb);
        }

        let value_register = self.resolve_to_register(Some(value_type)).unwrap();

        self.locked_register = Some(value_register);
        let dynamic_offset = self.resolve_with_target(None);

        let value_location = match value_register {
            Register::GP(reg) => MapperLocation::GPRegister(reg),
            Register::Pair(pair) => MapperLocation::RegisterPair(pair),
            Register::FP(reg) => MapperLocation::FPRegister {
                reg,
                val_size: match value_type {
                    ValueType::F32 => ValueSize::Word,
                    ValueType::F64 => ValueSize::DoubleWord,
                    _ => panic!("FP register selected for non-FP value type"),
                },
            },
        };
        let static_offset = u32::try_from(memarg.offset)
            .expect("memory offset does not fit in 32 bits");

        let used_registers = vec![value_location.clone()];
        let mut scratch_variable_map = vec![];
        let dynamic_offset_register = dynamic_offset.map_to_gp_register(None, self, &mut scratch_variable_map, &used_registers);

        let address = self.next_available_gp_register(
            &mut scratch_variable_map,
            &vec![MapperLocation::GPRegister(dynamic_offset_register)],
        );

        self.locked_register = None;


        self.push_instruction(RiscvInstr::ADDI {
            rd: address,
            rs1: dynamic_offset_register,
            imm: Const12::new(0),
        });

        if static_offset != 0 {
            if (-2048..=2047).contains(&(static_offset as i32)) {
                self.push_instruction(RiscvInstr::ADDI {
                    rd: address,
                    rs1: address,
                    imm: Const12::new(static_offset as i16),
                });
            } else {
                let offset_register = MapperLocation::Immediate(Immediate::I32(static_offset)).map_to_gp_register(
                    None,
                    self,
                    &mut scratch_variable_map,
                    &vec![MapperLocation::GPRegister(address)],
                );
                self.push_instruction(RiscvInstr::ADD {
                    rd: address,
                    rs1: address,
                    rs2: offset_register,
                });
            }
        }

        #[cfg(feature = "address-masking")]
        self.push_instruction(RiscvInstr::AND {
            rd: address,
            rs1: address,
            rs2: ADDRESS_MASK,
        });

        self.push_instruction(RiscvInstr::ADD {
            rd: address,
            rs1: address,
            rs2: LINEAR_MEMORY_BASE,
        });

        match (value_register, value_type, mem_size) {
            (Register::GP(value), ValueType::I32, Memsize::Byte) => {
                self.push_instruction(RiscvInstr::SB { rs1: address, rs2: value, imm: Const12::new(0) });
            }
            (Register::GP(value), ValueType::I32, Memsize::HalfWord) => {
                self.push_instruction(RiscvInstr::SH { rs1: address, rs2: value, imm: Const12::new(0) });
            }
            (Register::GP(value), ValueType::I32, Memsize::Word) => {
                self.push_instruction(RiscvInstr::SW { rs1: address, rs2: value, imm: Const12::new(0) });
            }
            (Register::Pair(pair), ValueType::I64, Memsize::Byte) => {
                self.push_instruction(RiscvInstr::SB { rs1: address, rs2: pair.low, imm: Const12::new(0) });
            }
            (Register::Pair(pair), ValueType::I64, Memsize::HalfWord) => {
                self.push_instruction(RiscvInstr::SH { rs1: address, rs2: pair.low, imm: Const12::new(0) });
            }
            (Register::Pair(pair), ValueType::I64, Memsize::Word) => {
                self.push_instruction(RiscvInstr::SW { rs1: address, rs2: pair.low, imm: Const12::new(0) });
            }
            (Register::Pair(pair), ValueType::I64, Memsize::DoubleWord) => {
                self.push_instruction(RiscvInstr::SW { rs1: address, rs2: pair.low, imm: Const12::new(0) });
                self.push_instruction(RiscvInstr::SW { rs1: address, rs2: pair.high, imm: Const12::new(4) });
            }
            (Register::FP(value), ValueType::F32, Memsize::Word) => {
                self.push_instruction(RiscvInstr::FSW { rs1: address, rs2: value, imm: Const12::new(0) });
            }
            (Register::FP(value), ValueType::F64, Memsize::DoubleWord) => {
                self.push_instruction(RiscvInstr::FSD { rs1: address, rs2: value, imm: Const12::new(0) });
            }
            _ => panic!("unsupported store combination for store_value_in_memory"),
        }
    }

    /// Helper to check if we're in dead code and return early if so.
    /// 
    /// Dead code occurs after unconditional jumps (br, return, unreachable, br_table)
    /// and needs to be tracked through nested blocks.
    fn check_dead_code(&self) -> bool {
        *self.dead_code_flag_stack.last().unwrap_or(&false)
    }

    /// Helper to set dead code flag and truncate VB stack (used after unconditional jumps).
    /// 
    /// When we encounter an unconditional jump:
    /// 1. Mark current block as dead code
    /// 2. Truncate VB stack to block entry state (no more value expressions will be consumed)
    fn set_dead_code_and_truncate_vb_stack(&mut self) {
        if let Some(flag) = self.dead_code_flag_stack.last_mut() {
            *flag = true;
        }
        self.vb_stack.truncate(*(self.vb_stack_ptr_stack.last()).unwrap());
    }

//     /// Helper to save and restore a VB for conditional branches.
//     /// 
//     /// For conditional branches that may have a result:
//     /// - Save the result VB expression before branching
//     /// - Restore it after the branch for the fall-through case
//     fn save_vb_for_branch(&mut self, has_result: bool) -> Option<VB> {
//         match has_result {
//             true => self.vb_stack.last().cloned(),
//             false => None,
//         }
//     }

//     /// Helper to restore VB after conditional branch.
//     /// 
//     /// Restores the saved VB expression for the fall-through path of a conditional branch.
//     fn restore_vb_after_branch(&mut self, vb: Option<VB>) {
//         vb.map(|vb| self.vb_stack.push(vb));
//     }

    /// Helper to calculate branch target index from relative depth.
    /// 
    /// WebAssembly branches use relative depths:
    /// - 0 = current block
    /// - 1 = parent block
    /// - etc.
    /// 
    /// This converts to absolute index in our block result stack.
    fn calculate_branch_target_index(&self, relative_depth: u32) -> i32 {
        self.cfg_block_result_stack.len() as i32 - (1 + relative_depth as i32)
    }

//     fn handle_function_return_branch(&mut self) {
//         let return_type = self.get_current_function_result_type();
//         let last_vb = match return_type {
//             Some(..) => self.vb_stack.last().cloned(),
//             None => None,
//         };
//         self.resolve_return_value();
//         self.push_instruction(Instr::RET);
//         self.restore_vb_after_branch(last_vb);
//     }
//
//     fn handle_block_branch(&mut self, index: usize) {
//         let label_state = self.cfg_block_result_stack.get(index).unwrap().label_state;
//         let last_vb = self.save_vb_for_branch(label_state.result_size.is_some());
//         self.resolve_block_result(label_state);
//         self.restore_vb_after_branch(last_vb);
//         let target = match self.cfg_label_stack[index] {
//             BlockLabel::BlockLoop(index) => index,
//             BlockLabel::If { else_label: _, end_label, .. } => end_label,
//         };
//         self.generate_jump_instruction(target);
//     }

    /// Helper to generate appropriate jump instruction based on label state
    fn generate_cfg_jump(&mut self, target_label: LabelIndex) {
        let instruction_hw_index = self.wasm_runtime.instruction_hw_count;
        self.push_instruction(RiscvInstr::JAL {
                rd: GPRegister(0),
                imm: Const20::new(0),
            });
        self.cfg_jobs.push(CfgJob {
            instruction_hw_index,
            target_label,
        });
    }

    /// Materializes the stack pointer at the modeled stack height.
    ///
    /// The callee derives its frame pointer from the stack pointer on entry, and its parameters
    /// are addressed relative to that frame pointer, so the stack pointer has to point exactly at
    /// the top of the argument block when the call is executed.
    fn sync_stack_pointer_to_modeled_height(&mut self) {
        let imm = self.stack_logical_offset_to_imm(self.modeled_stack_height);
        if imm != 0 {
            self.push_instruction(RiscvInstr::ADDI {
                rd: STACK_POINTER,
                rs1: STACK_POINTER,
                imm: Const12::new(imm),
            });
            self.stack_pointer_height = self.modeled_stack_height;
        }
    }

    /// Remove parameters from the VB stack as well as the memory stack after a function call
    fn cleanup_function_call_parameters(&mut self, function_type: &wasmparser::FuncType) {
        let params_count = function_type.params().len();
        self.vb_stack.truncate(self.vb_stack.len() - params_count);
        // The callee's epilogue restores sp = fp_callee = top of arg block, so no sp adjustment needed.
        let params_size: usize = function_type.params().iter().map(|ty| val_type_size(ty).as_bytes()).sum();
        self.modeled_stack_height = self.modeled_stack_height.sub(params_size);
    }

    /// Push function result onto the stack and add it to the VB stack
    fn handle_function_call_result(&mut self, function_type: &wasmparser::FuncType) {
        let Some(ty) = function_type.results().get(0).copied() else {
            return;
        };

        let val_type = ValueType::from_wasmparser(&ty);
        let offset = self.modeled_stack_height.add(val_type.as_bytes());
        let imm = self.stack_logical_offset_to_imm(offset);

        match val_type {
            ValueType::I32 => self.push_instruction(RiscvInstr::SW {
                rs1: STACK_POINTER,
                rs2: GPRegister(10),
                imm: Const12::new(imm),
            }),
            ValueType::I64 => {
                self.push_instruction(RiscvInstr::SW {
                    rs1: STACK_POINTER,
                    rs2: GPRegister(10),
                    imm: Const12::new(imm),
                });
                self.push_instruction(RiscvInstr::SW {
                    rs1: STACK_POINTER,
                    rs2: GPRegister(11),
                    imm: Const12::new(imm + 4),
                });
            }
            ValueType::F32 => self.push_instruction(RiscvInstr::FSW {
                rs1: STACK_POINTER,
                rs2: FPRegister(10),
                imm: Const12::new(imm),
            }),
            ValueType::F64 => self.push_instruction(RiscvInstr::FSD {
                rs1: STACK_POINTER,
                rs2: FPRegister(10),
                imm: Const12::new(imm),
            }),
        }

        self.push_vb_stack(VB::AtomicVB(AtomicVB::Resolved { val_type, offset }));
    }

//     /// Helper to load a 32-bit pointer into an address register
//     fn load_pointer_to_address_register(&mut self, ptr: u32, dest: AddressRegister) {
//         let ptr_upper = (ptr.wrapping_add(0x8000) >> 16) as u16; // From Aurix manual volume 2 1.7 Address arithmetic, solves sign extension for the lower offset
//         let ptr_lower = ptr as u16;
//         self.push_instruction(Instr::MOVHA { src: Const16::new(ptr_upper), dest });
//         self.push_instruction(Instr::LEA { base: dest, offset: Const16::new(ptr_lower), dest });
//     }

//     /// Helper to calculate total size of function parameters in bytes
//     fn calculate_params_size(function_type: &wasmparser::FuncType) -> u16 {
//         function_type.params().iter().map(|ty| val_type_size(ty).as_bytes() as u16).sum()
//     }

    /// Helper to update label addresses for forward jumps
    fn update_label_addresses(&mut self, block_label: BlockLabel) {
        let label_indices = match block_label {
            // index is the index of the label in cfg_label_map
            BlockLabel::BlockLoop(index) => vec![index],
            //encountered in case of an if construct without an else
            BlockLabel::If { else_label, end_label, .. } => vec![else_label, end_label],
        };

        label_indices.iter().for_each(|&label_index| {
            self.update_label_to_current_position(label_index);
        });
    }

//     /// Helper to handle function pointer loading for indirect calls
//     fn load_function_pointer(&mut self, function_label: u32) {
//         let fun_ptr_lower = function_label as u16;
//         let fun_ptr_upper = (function_label.wrapping_add(0x8000) >> 16) as u16; // From Aurix manual volume 2 1.7 Address arithmetic, solves sign extension for the lower offset
//         self.push_instruction(Instr::MOVHA { src: Const16(fun_ptr_upper), dest: ADDRESS_ACCUMULATOR });
//         self.push_instruction(Instr::LEA { base: ADDRESS_ACCUMULATOR, offset: Const16(fun_ptr_lower), dest: ADDRESS_ACCUMULATOR });
//         self.push_instruction(Instr::CALLI { target: ADDRESS_ACCUMULATOR });
//     }

    // ================================================================================
    // STACK AND MEMORY MANAGEMENT HELPERS
    // ================================================================================

    /// Helper to get current runtime stack offset
    fn get_current_stack_offset(&self) -> StackHeight {
        self.modeled_stack_height
    }

    /// Helper to get runtime stack offset from last VB on stack
    fn get_runtime_stack_offset_from_last_vb(&self) -> StackHeight {
        match self.vb_stack.last() {
            Some(VB::AtomicVB(AtomicVB::Resolved { offset, .. })) => *offset,
            _ => StackHeight::from(0),
        }
    }

    // ================================================================================
    // FUNCTION TYPE AND METADATA HELPERS
    // ================================================================================

    /// Helper to get current function's result type
    fn get_current_function_result_type(&self) -> Option<&ValType> {
        self.wasm_runtime.types[self.function_type_index as usize].unwrap_func().results().get(0)
    }

    /// Helper to get function type by function index
    fn get_function_type(&self, function_index: u32) -> wasmparser::FuncType {
        self.wasm_runtime.types[self.global_translator.function_type_map[function_index as usize] as usize].unwrap_func().clone()
    }

    /// Helper to get function type by type index  
    fn get_function_type_by_type_index(&self, type_index: u32) -> wasmparser::FuncType {
        self.wasm_runtime.types[type_index as usize].unwrap_func().clone()
    }

    // ================================================================================
    // VB STACK MANIPULATION HELPERS
    // ================================================================================

    /// Helper to resolve a value expression to a scratch general purpose register
    fn resolve_to_gp_register(&mut self) -> GPRegister {
        self.resolve_with_target(None).map_to_gp_register(None, self, &mut vec![], &vec![])
    }

    /// Helper to resolve a value expression to a scratch register pair.
    fn resolve_to_register_pair(&mut self) -> RegisterPair {
        self.resolve_with_target(None).map_to_register_pair(None, self, &mut vec![], &vec![])
    }

    /// Helper to resolve a value expression to a scratch floating-point register.
    fn resolve_to_fp_register(&mut self, value_type: ValueType) -> FPRegister {
        let val_size = match value_type {
            ValueType::F32 => ValueSize::Word,
            ValueType::F64 => ValueSize::DoubleWord,
            _ => panic!("resolve_to_fp_register expects an FP value type"),
        };
        self.resolve_with_target(None).map_to_fp_register(None, val_size, self, &mut vec![], &vec![])
    }

    /// Helper to resolve a value expression to an appropriate typed register location.
    fn resolve_to_register(&mut self, value_type: Option<ValueType>) -> Option<Register> {
        value_type.map(|ty| match ty {
            ValueType::I32 => Register::GP(self.resolve_to_gp_register()),
            ValueType::I64 => Register::Pair(self.resolve_to_register_pair()),
            ValueType::F32 => Register::FP(self.resolve_to_fp_register(ValueType::F32)),
            ValueType::F64 => Register::FP(self.resolve_to_fp_register(ValueType::F64)),
        })
    }

    /// Helper to pop VB expression from stack
    fn pop_vb(&mut self) -> VB {
        self.pop_vb_stack()
    }

//     // ================================================================================
//     // FUNCTION CALL HELPERS
//     // ================================================================================

    /// Helper to setup function call context (resolve all, save context)
    fn setup_function_call(&mut self) {
        self.resolve_all();
        
        // Spill every register-homed body local to its backup frame slot.
        // The callee is free to clobber these registers; we save them preemptively.
        let spills: Vec<_> = self.locals_map[self.num_params..].iter().cloned().zip(self.local_save_slots.iter().copied()).collect();
        for (loc, slot) in spills {
            match loc {
                MapperLocation::GPRegister(r) => {
                    self.push_instruction(RiscvInstr::SW { rs1: FRAME_POINTER, rs2: r, imm: Const12::new(slot) });
                }
                MapperLocation::RegisterPair(p) => {
                    self.push_instruction(RiscvInstr::SW { rs1: FRAME_POINTER, rs2: p.low,  imm: Const12::new(slot) });
                    self.push_instruction(RiscvInstr::SW { rs1: FRAME_POINTER, rs2: p.high, imm: Const12::new(slot + 4) });
                }
                MapperLocation::FPRegister { reg, val_size: ValueSize::Word } => {
                    self.push_instruction(RiscvInstr::FSW { rs1: FRAME_POINTER, rs2: reg, imm: Const12::new(slot) });
                }
                MapperLocation::FPRegister { reg, val_size: ValueSize::DoubleWord } => {
                    self.push_instruction(RiscvInstr::FSD { rs1: FRAME_POINTER, rs2: reg, imm: Const12::new(slot) });
                }
                _ => {}
            }
        }
        
        // Remap all register-homed locals (both parameters and body locals) to their frame locations.
        // This prevents the callee from corrupting values via registers. Parameters are stored on the
        // caller's stack at positive fp-relative offsets; body locals use backup slots at negative offsets.
        // After the call, subsequent loads of these locals will fetch from the frame instead of stale registers.
        
        let function_type = self.wasm_runtime.types[self.function_type_index as usize].unwrap_func();
        let params_type: Vec<ValueType> = function_type
            .params()
            .iter()
            .map(ValueType::from_wasmparser)
            .collect();
        
        // Compute parameter frame offsets (parameters are stored on caller stack in reverse order).
        let mut param_offsets = vec![0i16; params_type.len()];
        let mut offset = 0;
        for i in (0..params_type.len()).rev() {
            param_offsets[i] = offset;
            offset += params_type[i].as_bytes() as i16;
        }
        
        // Single loop: remap all locals that are register-homed to their frame locations.
        for (i, loc) in self.locals_map.iter_mut().enumerate() {
            let frame_offset = if i < self.num_params {
                param_offsets[i]  // Parameter: use its offset on caller stack
            } else {
                self.local_save_slots[i - self.num_params]  // Body local: use backup slot
            };
            
            match loc {
                MapperLocation::GPRegister(_) => {
                    *loc = MapperLocation::Frame { val_type: ValueType::I32, offset: frame_offset };
                }
                MapperLocation::RegisterPair(_) => {
                    *loc = MapperLocation::Frame { val_type: ValueType::I64, offset: frame_offset };
                }
                MapperLocation::FPRegister { val_size: ValueSize::Word, .. } => {
                    *loc = MapperLocation::Frame { val_type: ValueType::F32, offset: frame_offset };
                }
                MapperLocation::FPRegister { val_size: ValueSize::DoubleWord, .. } => {
                    *loc = MapperLocation::Frame { val_type: ValueType::F64, offset: frame_offset };
                }
                _ => {}  // Already frame-based or other location; leave as-is.
            }
        }
        
        self.sync_stack_pointer_to_modeled_height();
    }

    fn cleanup_function_call(&mut self, _arg_bytes: usize) {

        // The callee may clobber STACK_BASE and FRAME_POINTER; rebuild them from caller state.
        // At this point SP is still at the pre-call top-of-arg-block, i.e. SP = STACK_BASE - stack_pointer_height.
        let scratch = GPRegister(24);
        add_i32_imm(
            self,
            STACK_BASE,
            STACK_POINTER,
            self.stack_pointer_height.to_i32(),
            scratch,
        );
        add_i32_imm(
            self,
            FRAME_POINTER,
            STACK_BASE,
            -(self.total_frame_offset as i32),
            scratch,
        );

        // Reload every register-homed local from its backup slot.
        let reloads: Vec<_> = self.locals_map[self.num_params..].iter().cloned().zip(self.local_save_slots.iter().copied()).collect();
        for (loc, slot) in reloads {
            match loc {
                MapperLocation::GPRegister(r) => {
                    self.push_instruction(RiscvInstr::LW { rd: r, rs1: FRAME_POINTER, imm: Const12::new(slot) });
                }
                MapperLocation::RegisterPair(p) => {
                    self.push_instruction(RiscvInstr::LW { rd: p.low,  rs1: FRAME_POINTER, imm: Const12::new(slot) });
                    self.push_instruction(RiscvInstr::LW { rd: p.high, rs1: FRAME_POINTER, imm: Const12::new(slot + 4) });
                }
                MapperLocation::FPRegister { reg, val_size: ValueSize::Word } => {
                    self.push_instruction(RiscvInstr::FLW { rd: reg, rs1: FRAME_POINTER, imm: Const12::new(slot) });
                }
                MapperLocation::FPRegister { reg, val_size: ValueSize::DoubleWord } => {
                    self.push_instruction(RiscvInstr::FLD { rd: reg, rs1: FRAME_POINTER, imm: Const12::new(slot) });
                }
                _ => {}
            }
        }
        
        // stack_pointer_height is intentionally NOT updated: sp was restored by the callee to
        // fp_caller - arg_bytes, which is exactly what stack_pointer_height already reflects.
    }


    /// Emits the br_table dispatch sequence.
    ///
    /// Contract:
    /// 1. `arm_labels` contains one label per arm, where the last label is the default arm.
    /// 2. `index_register` already contains the dynamic branch index.
    /// 3. This helper only emits dispatch, not arm bodies.
    ///
    /// Lowering strategy:
    /// 1. Emit one `BEQ` per explicit table arm comparing `index_register` with arm index.
    /// 2. Each `BEQ` gets a CFG fixup job targeting the corresponding arm label.
    /// 3. Emit an unconditional jump to the default arm when no comparison matched.
    fn generate_br_table_jump(&mut self, index_register: GPRegister, arm_labels: &[LabelIndex]) {
        debug_assert!(!arm_labels.is_empty(), "br_table requires at least one arm");

        // Emit a dispatch chain: compare dynamic index against each explicit arm index,
        // then jump to the default arm when no explicit entry matches.
        let explicit_arms = arm_labels.len() - 1;
        for (case_idx, &target_label) in arm_labels.iter().take(explicit_arms).enumerate() {
            let mut scratch_variable_map = vec![];
            let case_index_register = MapperLocation::Immediate(Immediate::I32(case_idx as u32)).map_to_gp_register(
                None,
                self,
                &mut scratch_variable_map,
                &vec![MapperLocation::GPRegister(index_register)],
            );

            let branch_hw_index = self.wasm_runtime.instruction_hw_count;
            self.push_instruction(RiscvInstr::BEQ {
                rs1: index_register,
                rs2: case_index_register,
                imm: Const12::new(0),
            });
            self.cfg_jobs.push(CfgJob {
                instruction_hw_index: branch_hw_index,
                target_label,
            });
        }

        let default_label = *arm_labels.last().unwrap();
        self.generate_cfg_jump(default_label);
    }
    

    /// Helper to handle local variable dependency resolution
    fn handle_local_dependency(&mut self, local_index: u32) {
        // Check if any existing VB expressions depend on this local variable
        let vb: VB = self.pop_vb();
        if self.vb_stack.iter().any(|vb| vb.depends_on_local(local_index)) {
            self.resolve_all();
        }
        self.push_vb_stack(vb);
    }

    /// Add a label to the cfg_label_map
    fn add_label(&mut self, label: Option<usize>) {
        self.cfg_label_map.push(label);
    }
    
    /// Add a label for the current instruction position
    fn add_current_position_label(&mut self) {
        self.add_label(Some(self.wasm_runtime.instruction_hw_count));
    }
    
    /// Add a placeholder label (None) for forward jumps
    fn add_placeholder_label(&mut self) {
        self.add_label(None);
    }
    
    /// Get the current length of the cfg_label_map (used for generating new label indices)
    fn get_current_label_index(&self) -> LabelIndex {
        LabelIndex::new(self.cfg_label_map.len() )
    }
    
    /// Update a label at the given index with the current instruction position
    fn update_label_to_current_position(&mut self, index: LabelIndex) {
        let index = index.to_usize();
        if self.cfg_label_map[index].is_some() {
            return; // Label already set, no need to update. This is a backward jump.
        }
        self.cfg_label_map[index] = Some(self.wasm_runtime.instruction_hw_count);
    }

    /// Helper to generate appropriate call instruction based on function index:
    /// CALL if the displacement is small enough, otherwise CALLI with the function pointer loaded into an address register.
    fn generate_call_instruction(&mut self, function_index: u32) {
        let function_label = self.wasm_runtime.function_labels.get(function_index as usize).copied();
        match function_label {
            Some(function_label) => {
                // Callee already translated: patch a JAL in place with the real displacement.
                let call_hw_index = self.wasm_runtime.instruction_hw_count;
                self.push_instruction(RiscvInstr::JAL { rd: GPRegister(1), imm: Const20::new(0) });
                let call_addr = (self.wasm_runtime.instructions.as_ptr() as u32) + (call_hw_index as u32 * 2);
                let disp = (function_label as i32).wrapping_sub(call_addr as i32);
                patch_jal_displacement(self.wasm_runtime, call_hw_index, disp);
            }
            None => {
                // Callee not yet translated: emit placeholder JAL and register a fixup job.
                let call_hw_index = self.wasm_runtime.instruction_hw_count;
                self.push_instruction(RiscvInstr::JAL { rd: GPRegister(1), imm: Const20::new(0) });
                self.global_translator.function_call_jobs.push(call_hw_index);
                // Encode function_index into the placeholder: replace_target_with_disp_call reads it as `instruction >> 8`.
                let low  = self.wasm_runtime.instructions[call_hw_index] as u32;
                let high = self.wasm_runtime.instructions[call_hw_index + 1] as u32;
                let word = (low | (high << 16) & 0x000000ff) | ((function_index) << 8);
                self.wasm_runtime.instructions[call_hw_index]     = (word & 0xffff) as u16;
                self.wasm_runtime.instructions[call_hw_index + 1] = ((word >> 16) & 0xffff) as u16;
            }
        }
    }

    /// Emits a bounds check (address-masking only) and type check for `call_indirect`.
    ///
    /// `idx_reg` holds the dynamic table index. Scratch registers t3/t4/t5 (x28-x30) are clobbered.
    /// After this call t3 holds the resolved canonical id (unused by caller; only side-effect is trap on mismatch).
    fn emit_call_indirect_type_check(&mut self, idx_reg: GPRegister, type_index: u32) {
        let t3 = GPRegister(28);
        let t4 = GPRegister(29);
        let t5 = GPRegister(30);

        // Bounds check (address-masking only): CEBREAK if idx >= table_size.
        // BLTU branches 6 bytes forward (over the 4-byte BLTU itself + 2-byte CEBREAK).
        #[cfg(feature = "address-masking")]
        {
            add_i32_imm(self, t4, GPRegister(0), self.global_translator.table_size as i32, t3);
            self.push_instruction(RiscvInstr::BLTU { rs1: idx_reg, rs2: t4, imm: Const12::new(6) });
            self.push_instruction(RiscvInstr::CEBREAK);
        }

        // t3 = table_type_indices[idx_reg]: raw type index of the callee.
        let tti_ptr = self.wasm_runtime.table_type_indices.as_ptr() as u32;
        let hi = (tti_ptr.wrapping_add(0x800) >> 12) as i32;
        let lo = (tti_ptr & 0xFFF) as i16;
        self.push_instruction(RiscvInstr::LUI  { rd: t4, imm: Const20::new(hi) });
        if lo != 0 { self.push_instruction(RiscvInstr::ADDI { rd: t4, rs1: t4, imm: Const12::new(lo) }); }
        self.push_instruction(RiscvInstr::SLLI { rd: t3, rs1: idx_reg, imm: ShAmt5::new(2) });
        self.push_instruction(RiscvInstr::ADD  { rd: t3, rs1: t4,      rs2: t3 });
        self.push_instruction(RiscvInstr::LW   { rd: t3, rs1: t3,      imm: Const12::new(0) });

        // t3 = type_canonical_ids[t3]: canonical equivalence class of the callee's type.
        let tci_ptr = self.wasm_runtime.type_canonical_ids.as_ptr() as u32;
        let hi = (tci_ptr.wrapping_add(0x800) >> 12) as i32;
        let lo = (tci_ptr & 0xFFF) as i16;
        self.push_instruction(RiscvInstr::LUI  { rd: t4, imm: Const20::new(hi) });
        if lo != 0 { self.push_instruction(RiscvInstr::ADDI { rd: t4, rs1: t4, imm: Const12::new(lo) }); }
        self.push_instruction(RiscvInstr::SLLI { rd: t3, rs1: t3, imm: ShAmt5::new(2) });
        self.push_instruction(RiscvInstr::ADD  { rd: t3, rs1: t4, rs2: t3 });
        self.push_instruction(RiscvInstr::LW   { rd: t3, rs1: t3, imm: Const12::new(0) });

        // Compare callee's canonical id with the statically required one; trap on mismatch.
        // BEQ branches 6 bytes forward (over itself + CEBREAK) on match.
        let target_canonical = self.wasm_runtime.type_canonical_ids[type_index as usize];
        add_i32_imm(self, t4, GPRegister(0), target_canonical as i32, t5);
        self.push_instruction(RiscvInstr::BEQ  { rs1: t3, rs2: t4, imm: Const12::new(6) });
        self.push_instruction(RiscvInstr::CEBREAK);
    }

}

#[allow(unused_variables)]
impl <'a> VisitOperator <'a> for Translator<'a,'_>{
    
    type Output = ();
    
    wasmparser::for_each_operator!(_visit_only_mvp);

    // ================================================================================
    // CONTROL FLOW INSTRUCTIONS
    // ================================================================================

    fn visit_unreachable(&mut self) {
        if self.check_dead_code() {
            return;
        }
        // TODO: We should not generate a trap instruction, instead have the result of the original function be an error
        // We should store the stack state before the original function call
        // self.push_instruction(RiscvInstr::CEBREAK);
        self.set_dead_code_and_truncate_vb_stack();
    }

    fn visit_nop(&mut self) {
        // Do nothing
    }

    fn visit_block(&mut self,blockty:BlockType) {
        self.enter_block(blockty, BlockStyle::Block);
    }

    fn visit_loop(&mut self,blockty:BlockType) {
        self.enter_block(blockty, BlockStyle::Loop);
    }

    fn visit_if(&mut self,blockty:BlockType) {

        //dead code handling
        if self.check_dead_code() {
            // in case the if block is not inside dead code, this is handled inside enter_block
            self.dead_code_flag_stack.push(true);
            return;
        }

        // condition VB is popped out so that it does NOT get resolved or considered 
        // as part of the state of the VB stack prior to the entry.
        let condition_vb = self.vb_stack.pop().unwrap();
        self.enter_block(blockty, BlockStyle::Block);
        self.vb_stack.push(condition_vb);

        // resolve condition
        let condition_register = self.resolve_to_gp_register();
        self.sync_stack_pointer_to_modeled_height();

        // creating label for beginning of else block
        // note that a label for the end of the if block (after the else part) has been already created within enter_block
        let else_label = self.get_current_label_index();

        //generate jump instruction to the beginning of else block if condition is equal to 0 (i.e it is false)
        let branch_hw_index = self.wasm_runtime.instruction_hw_count;
        self.push_instruction(RiscvInstr::BEQ { rs1: condition_register, rs2: GPRegister(0), imm: Const12::new(0) });
        self.cfg_jobs.push(CfgJob { instruction_hw_index: branch_hw_index, target_label: else_label });

        // replace the label stack last item (inserted within enter_block) to include both the else label (the newly created one) and the end label
        // (created within enter_block)
        if let Some(label) = self.cfg_label_stack.last_mut() {
            let end_label = match label {
                BlockLabel::BlockLoop(index) => *index,
                _ => panic!("Expected block label")
            };
            *label = BlockLabel::If { else_label, end_label };
        }

        // placeholder for the address of the beginning of the else block
        self.add_placeholder_label();

    }

    
    fn visit_else(&mut self){ 
        //an else instruction is encountered.
        //2 dead code flags exist

        // 1) is the code right before the else instruction (i.e.) inside the if block dead code ?
        let inside_dead_code_flag = self.dead_code_flag_stack.pop().unwrap_or(false);

        // 2) is the whole if block (inside) dead code ?
        let outside_dead_flag_code = *self.dead_code_flag_stack.last().unwrap_or(&false);
        // initial state of the dead code flag inside the block matches
        // initially whether the whole block is inside dead code
        self.dead_code_flag_stack.push(outside_dead_flag_code);

        if outside_dead_flag_code {
            return;
        }

        // pop labels from the label stack pushed at the beginning of the if block
        let (else_label, end_label) = match self.cfg_label_stack.pop().unwrap() {
            BlockLabel::If { else_label, end_label } => (else_label, end_label),
            _ => panic!("Expected if label")
        };

        // resolve the if block unless it was dead code (if it was dead code, you met a br and it was already resolved)
        // then add the jump instruction to the end of the if block (same thing, if it was dead code you already did the jump)
        let end_state = self.cfg_block_result_stack.last().unwrap().end_state;
        if !inside_dead_code_flag {
            self.resolve_block_result(end_state);
            self.generate_cfg_jump(end_label);
        }

        // else label address is the current position in the instructions, directly after the jump
        self.update_label_to_current_position(else_label);

        // Reset to block entry state: result hasn't been produced yet on the else-path.
        self.modeled_stack_height = end_state.height.sub(end_state.result_type.map(|t| t.as_bytes()).unwrap_or(0));
        self.stack_pointer_height = self.modeled_stack_height; // TODO: should be a call to sync_stack_pointer_to_modeled_height

        // start of the else branch: replace the if label (that was already popped) with a block label
        // else label is irrelevant at this point and the rest can be treated like a block instruction.
        self.cfg_label_stack.push(BlockLabel::BlockLoop(end_label));
    }

    // end instruction matches the end of if/else, block, and loop constructs as well as the end of a function
    // the behavior is generalized to cover all of the possibilities based on the information collected
    // while passing through the wasm instructions
    fn visit_end(&mut self){        
        // dead code flag of whether code before the end instruction inside the innermost construct is 
        // dead code.
        let inside_dead_code_flag = self.dead_code_flag_stack.pop().unwrap_or(false);

        // dead code flag indicating whether the whole construct was inside a dead code zone
        let outside_dead_flag_code = *self.dead_code_flag_stack.last().unwrap_or(&false);
        if outside_dead_flag_code {
            return;
        }

        // pop the block result from the stack
        let end_state = self.cfg_block_result_stack.pop().map(|block_result| block_result.end_state);

        if !inside_dead_code_flag {
            // return block result or function return value.
            // end of a function is distinguished through an empty block state stack -> end_state is None
            match end_state{
                Some(end_state) => self.resolve_block_result(end_state),
                None => {
                    self.emit_function_return_sequence();
                }
            }
        } else if let Some(end_state) = end_state {
            // Body was dead code (e.g. ended with return/br), but branches targeting end_label
            // synced SP to end_state.height. Update compiler tracking to match.
            self.modeled_stack_height = end_state.height; // TODO: should be a call to sync_stack_pointer_to_modeled_height() instead of direct assignment?
            self.stack_pointer_height = end_state.height;
        }

        // update label addresses (previously unknown) for forward jump labels to current instruction count
        // and pop value from stack
        if let Some(block_label) = self.cfg_label_stack.pop() {
            self.update_label_addresses(block_label);
        }

        // current construct/block is ending. Pop value from stack
        let _ = self.vb_stack_ptr_stack.pop();

        // result of a block is returned on the runtime stack
        // and on a virtual level is on top of the operand stack now
        // a VB is pushed on top of the VB stack to update it with the location of new value.
        if let Some(StackState { height: offset, result_type: Some(val_type) }) = end_state {
            self.vb_stack.push(VB::AtomicVB(AtomicVB::Resolved { val_type, offset }));
        }

    }

    fn visit_br(&mut self,relative_depth:u32){
        if self.check_dead_code() {
            return;
        }

        let index = self.calculate_branch_target_index(relative_depth);
        if index < 0 {
            self.emit_function_return_sequence();
        } else { 
            let label_state = self.cfg_block_result_stack.get(index as usize).map(|block_result| block_result.label_state).unwrap();
            self.resolve_block_result(label_state);

            let target_label = match self.cfg_label_stack[index as usize] {
                BlockLabel::BlockLoop(index) => index,
                BlockLabel::If { else_label: _, end_label, .. } => end_label,
            };
    
            self.generate_cfg_jump(target_label);
        }

        self.set_dead_code_and_truncate_vb_stack();
    }

    fn visit_br_if(&mut self, relative_depth: u32) {
        if self.check_dead_code() {
            return;
        }

        let condition_register = self.resolve_to_gp_register();
        self.sync_stack_pointer_to_modeled_height(); // sp == modeled at snapshot point
        let index = self.calculate_branch_target_index(relative_depth);

        let has_result = if index < 0 {
            self.get_current_function_result_type().is_some()
        } else {
            self.cfg_block_result_stack[index as usize].label_state.result_type.is_some()
        };

        // Snapshot state after condition pop; restored for the fallthrough path.
        let state_before = self.modeled_stack_height; // equals stack_pointer_height after sync
        let saved_result_vb: Option<VB> = if has_result { self.vb_stack.last().cloned() } else { None };

        let skip_label = self.get_current_label_index();
        let branch_hw_index = self.wasm_runtime.instruction_hw_count;
        self.push_instruction(RiscvInstr::BEQ { rs1: condition_register, rs2: GPRegister(0), imm: Const12::new(0) });
        self.cfg_jobs.push(CfgJob { instruction_hw_index: branch_hw_index, target_label: skip_label });
        self.add_placeholder_label();

        if index < 0 {
            self.emit_function_return_sequence();
        } else {
            let label_state = self.cfg_block_result_stack[index as usize].label_state;
            self.resolve_block_result(label_state);

            let target_label = match self.cfg_label_stack[index as usize] {
                BlockLabel::BlockLoop(idx) => idx,
                BlockLabel::If { end_label, .. } => end_label,
            };

            self.generate_cfg_jump(target_label);
        }

        self.modeled_stack_height = state_before;
        self.stack_pointer_height = state_before;
        if let Some(vb) = saved_result_vb {
            self.vb_stack.push(vb);
        }

        self.update_label_to_current_position(skip_label);
    }

    /// Translates a WebAssembly `br_table` as an unconditional multi-way branch.
    ///
    /// Step-by-step behavior:
    /// 1. Early-return when currently inside dead code.
    /// 2. Preserve stack evaluation order by resolving all VBs except the dynamic index first.
    /// 3. Resolve the dynamic index to a GP register and sync SP to modeled height.
    /// 4. Snapshot `(modeled_stack_height, stack_pointer_height)` as the per-arm entry state.
    /// 5. Materialize one placeholder CFG label per arm (`targets` + `default`).
    /// 6. Emit dispatch with `generate_br_table_jump`.
    /// 7. For each arm label:
    ///    - bind the label to current position,
    ///    - resolve branch destination exactly like `visit_br`,
    ///    - emit jump/return path,
    ///    - restore the entry snapshot so the next arm is emitted from identical state.
    /// 8. Mark subsequent code as dead because `br_table` never falls through.
    ///
    /// Invariant: every arm starts with the same logical stack state and stack pointer tracking.
    fn visit_br_table(&mut self, targets:BrTable<'a>) {
        if self.check_dead_code() {
            return;
        }

        // resolving all VBs prior to the last one (dynamic index) and pushing them to the stack
        let index_vb = self.vb_stack.pop().unwrap();
        self.resolve_all();
        self.vb_stack.push(index_vb);

        // resolving the dynamic index to a register
        let index_register = self.resolve_to_gp_register();
        self.sync_stack_pointer_to_modeled_height();

        // Snapshot state after consuming br_table index. Each arm starts from this state.
        let state_before = self.modeled_stack_height;

        let mut branch_targets: Vec<u32> = targets.targets().map(Result::unwrap).collect();
        branch_targets.push(targets.default());

        let arm_labels: Vec<LabelIndex> = (0..branch_targets.len())
            .map(|_| {
                let label = self.get_current_label_index();
                self.add_placeholder_label();
                label
            })
            .collect();

        self.generate_br_table_jump(index_register, &arm_labels);

        for (&arm_label, target_depth) in arm_labels.iter().zip(branch_targets.into_iter()) {
            self.update_label_to_current_position(arm_label);

            let branch_index = self.calculate_branch_target_index(target_depth);
            let has_result = if branch_index < 0 {
                self.get_current_function_result_type().is_some()
            } else {
                self.cfg_block_result_stack[branch_index as usize].label_state.result_type.is_some()
            };
            let saved_result_vb = if has_result { self.vb_stack.last().cloned() } else { None };

            if branch_index < 0 {
                self.emit_function_return_sequence();
            } else {
                let label_state = self.cfg_block_result_stack[branch_index as usize].label_state;
                self.resolve_block_result(label_state);

                let target_label = match self.cfg_label_stack[branch_index as usize] {
                    BlockLabel::BlockLoop(index) => index,
                    BlockLabel::If { else_label: _, end_label, .. } => end_label,
                };

                self.generate_cfg_jump(target_label);
            }

            // Keep compile-time stack tracking aligned for the next emitted arm body.
            self.modeled_stack_height = state_before;
            self.stack_pointer_height = state_before;
            if let Some(vb) = saved_result_vb {
                self.vb_stack.push(vb);
            }
        }

        // br_table always transfers control and never falls through to subsequent instructions.
        self.modeled_stack_height = state_before;
        self.stack_pointer_height = state_before;

        self.set_dead_code_and_truncate_vb_stack();

    }

    fn visit_return(&mut self)  {
        if self.check_dead_code() {
            return;
        }

        self.emit_function_return_sequence();
        self.set_dead_code_and_truncate_vb_stack();
    }

    fn visit_call(&mut self,function_index:u32)  {
        if self.check_dead_code() {
            return;
        }

        let function_type = self.get_function_type(function_index);
        let arg_bytes: usize = function_type.params().iter()
            .map(|ty| ValueType::from_wasmparser(ty).as_bytes())
            .sum();
        self.setup_function_call();

        self.generate_call_instruction(function_index);

        self.cleanup_function_call_parameters(&function_type);
        self.handle_function_call_result(&function_type);

        self.cleanup_function_call(arg_bytes);
    }

    /// Translates a `call_indirect` instruction.
    ///
    /// The operand stack at this point is `[... args | table_index]`, with `table_index` on top.
    /// The call sequence is:
    /// 1. Pop the dynamic table index VB off the VB stack before resolving arguments.
    /// 2. Call `setup_function_call` to resolve all VBs (placing call args on the runtime stack)
    ///    and remap all locals to their frame locations so the callee cannot corrupt them.
    /// 3. Materialise the table index into a register.
    /// 4. Emit a type check: look up the callee's type in `table_type_indices`, resolve it to a
    ///    canonical id via `type_canonical_ids`, and trap (`CEBREAK`) on mismatch.
    /// 5. Load the function pointer from `TABLE_BASE[idx * 4]` and call it via `CJALR`.
    /// 6. Call `cleanup_function_call` to rebuild `STACK_BASE`/`FRAME_POINTER` and reload body locals.
    fn visit_call_indirect(&mut self, type_index: u32, _table_index: u32) {
        if self.check_dead_code() {
            return;
        }

        let function_type = self.get_function_type_by_type_index(type_index);
        let arg_bytes: usize = function_type.params().iter()
            .map(|ty| ValueType::from_wasmparser(ty).as_bytes())
            .sum();

        // Pop dynamic index before settling arguments onto the runtime stack.
        let table_offset_vb = self.vb_stack.pop().unwrap();
        self.setup_function_call();

        // Materialise the dynamic table index now that all call args are on the stack.
        self.vb_stack.push(table_offset_vb);
        let idx_reg = self.resolve_to_gp_register();

        self.emit_call_indirect_type_check(idx_reg, type_index);

        // Load function pointer from table and call: TABLE_BASE[idx * 4].
        let t3 = GPRegister(28);
        self.push_instruction(RiscvInstr::SLLI  { rd: t3, rs1: idx_reg, imm: ShAmt5::new(2) });
        self.push_instruction(RiscvInstr::ADD   { rd: t3, rs1: TABLE_BASE, rs2: t3 });
        self.push_instruction(RiscvInstr::LW    { rd: t3, rs1: t3, imm: Const12::new(0) });
        self.push_instruction(RiscvInstr::CJALR { rs1: t3 });

        self.cleanup_function_call_parameters(&function_type);
        self.handle_function_call_result(&function_type);
        self.cleanup_function_call(arg_bytes);
    }

    // ================================================================================
    // STACK MANIPULATION INSTRUCTIONS
    // ================================================================================

    /// Pop the VB stack and if the value is on the runtime stack, remove it from there as well
    fn visit_drop(&mut self) {
        if self.check_dead_code() {
            return;
        }

        let _ = self.pop_vb();

    }

    fn visit_select(&mut self) {
        if self.check_dead_code() {
            return;
        }
        let selector = self.pop_vb();
        let rhs = self.pop_vb();
        let lhs = self.pop_vb();

        let lhs_type = lhs.val_type(&self.locals_map, &self.global_translator.globals_map);
        let rhs_type = rhs.val_type(&self.locals_map, &self.global_translator.globals_map);

        self.push_vb_stack(VB::Select {
            selector: Box::new(selector),
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            val_type: lhs_type,
        });
    }

    // ================================================================================
    // VARIABLE ACCESS INSTRUCTIONS
    // ================================================================================

    fn visit_local_get(&mut self,local_index:u32)  {
        self.add_atomic_vb(AtomicVB::Local {index: local_index})
    }

    fn visit_local_set(&mut self,local_index:u32) {
        if self.check_dead_code() {
            return;
        }
        self.handle_local_dependency(local_index);
        let _ = self.resolve_with_target(Some(&self.locals_map[local_index as usize].clone()));
    }

    fn visit_local_tee(&mut self,local_index:u32) {
        if self.check_dead_code() {
            return;
        }
        self.handle_local_dependency(local_index);
        let _ = self.resolve_with_target(Some(&self.locals_map[local_index as usize].clone()));
        self.add_atomic_vb(AtomicVB::Local {index: local_index});
    }

    fn visit_global_get(&mut self,global_index:u32) {
        self.add_atomic_vb(AtomicVB::Global {index: global_index})
    }

    fn visit_global_set(&mut self,global_index:u32) { 
        if self.check_dead_code() {
            return;
        }
        let (offset, val_type) = self.global_translator.globals_map[global_index as usize];
        let _ = self.resolve_with_target(Some(&MapperLocation::Global { offset, val_type }));
    }

    // ================================================================================
    // MEMORY ACCESS INSTRUCTIONS
    // ================================================================================

    fn visit_i32_load(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I32Load { offset: memarg.offset as Address, align: memarg.align });
    }

    fn visit_i64_load(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I64Load { offset: memarg.offset as Address, align: memarg.align });
    }

    fn visit_f32_load(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::F32Load { offset: memarg.offset as Address, align: memarg.align });
    }

    fn visit_f64_load(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::F64Load { offset: memarg.offset as Address, align: memarg.align });
    }

    fn visit_i32_load8_s(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I32Load8s { offset: memarg.offset as Address, align: memarg.align })
    }

    fn visit_i32_load8_u(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I32Load8u { offset: memarg.offset as Address, align: memarg.align })
    }

    fn visit_i32_load16_s(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I32Load16s { offset: memarg.offset as Address, align: memarg.align })
    }

    fn visit_i32_load16_u(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I32Load16u { offset: memarg.offset as Address, align: memarg.align })
    }

    fn visit_i64_load8_s(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I64Load8s { offset: memarg.offset as Address, align: memarg.align })
    }

    fn visit_i64_load8_u(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I64Load8u { offset: memarg.offset as Address, align: memarg.align })
    }

    fn visit_i64_load16_s(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I64Load16s { offset: memarg.offset as Address, align: memarg.align })
    }

    fn visit_i64_load16_u(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I64Load16u { offset: memarg.offset as Address, align: memarg.align })
    }

    fn visit_i64_load32_s(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I64Load32s { offset: memarg.offset as Address, align: memarg.align })
    }

    fn visit_i64_load32_u(&mut self,memarg:MemArg) {
        self.add_unary_vb(UnaryVB::I64Load32u { offset: memarg.offset as Address, align: memarg.align })
    }

    fn visit_i32_store(&mut self,memarg:MemArg) {
        self.store_value_in_memory(memarg, Memsize::Word, ValueType::I32);
    }

    fn visit_i64_store(&mut self,memarg:MemArg) {
        self.store_value_in_memory(memarg, Memsize::DoubleWord, ValueType::I64);
    }

    fn visit_f32_store(&mut self,memarg:MemArg) {
        self.store_value_in_memory(memarg, Memsize::Word, ValueType::F32);
    }

    fn visit_f64_store(&mut self,memarg:MemArg) {
        self.store_value_in_memory(memarg, Memsize::DoubleWord, ValueType::F64);
    }

    fn visit_i32_store8(&mut self,memarg:MemArg) {
        self.store_value_in_memory(memarg, Memsize::Byte, ValueType::I32);
    }

    fn visit_i32_store16(&mut self,memarg:MemArg) {
        self.store_value_in_memory(memarg, Memsize::HalfWord, ValueType::I32);
    }

    fn visit_i64_store8(&mut self,memarg:MemArg) {
        self.store_value_in_memory(memarg, Memsize::Byte, ValueType::I64);
    }

    fn visit_i64_store16(&mut self,memarg:MemArg) {
        self.store_value_in_memory(memarg, Memsize::HalfWord, ValueType::I64);
    }

    fn visit_i64_store32(&mut self,memarg:MemArg) {
        self.store_value_in_memory(memarg, Memsize::Word, ValueType::I64);
    }

    fn visit_memory_size(&mut self,_mem:u32) {
        if self.check_dead_code() {
            return;
        }
        
        self.add_atomic_vb(AtomicVB::MemorySize);
    }

    /// Emits inline code for `memory.grow` without a helper call.
    /// On success it updates global memory-size (offset 0 in `GLOBAL_BASE`) and returns the previous size.
    /// On failure (overflow or maximum-size breach) it returns `u32::MAX`.
    fn visit_memory_grow(&mut self,_mem:u32) {
        if self.check_dead_code() {
            return;
        }

        let offset_vb = self.pop_vb();
        self.resolve_all();
        self.push_vb_stack(offset_vb);

        let grow_size_register = self
            .resolve_with_target(Some(&MapperLocation::GPRegister(GPRegister(27))))
            .map_to_gp_register(Some(GPRegister(27)), self, &mut vec![], &vec![]);
        let old_size_register = GPRegister(28);
        let new_size_register = GPRegister(29);
        let result_register = GPRegister(30);

        // Load old memory size from global base (offset 0) into `old_size_register`.
        self.push_instruction(RiscvInstr::LW {rd: old_size_register, rs1: GLOBAL_BASE, imm: Const12::new(0)});
        
        // Compute new size, then check for overflow: if new size < old size, set failure flag in `result_register`.
        self.push_instruction(RiscvInstr::ADD { rd: new_size_register, rs1: old_size_register, rs2: grow_size_register});
        self.push_instruction(RiscvInstr::SLTU { rd: result_register, rs1: new_size_register, rs2: old_size_register });

        // Load max size into `grow_size_register` and compare with new size. If new size > max, set failure flag.
        let max_size = MapperLocation::Immediate(Immediate::I32(self.global_translator.memory_size_limit))
        .map_to_gp_register(Some(grow_size_register),self,&mut vec![],&vec![]);

        self.push_instruction(RiscvInstr::SLTU { rd: max_size, rs1: max_size, rs2: new_size_register});

        // If either overflow or max-size breach occurred, set `result_register` to -1 (failure). Otherwise, set it to 0 (success).
        self.push_instruction(RiscvInstr::OR { rd: max_size, rs1: max_size, rs2: result_register});
        self.push_instruction(RiscvInstr::ADDI { rd: result_register, rs1: GPRegister(0), imm: Const12::new(-1)});

        // If failure flag != 0, skip the success block below.
        self.push_instruction(RiscvInstr::BNE { rs1: max_size, rs2: GPRegister(0), imm: Const12::new(12)});

        // Success block have the result be the old size, and update the value in global memory.
        self.push_instruction_no_compress(RiscvInstr::ADDI { rd: result_register, rs1: old_size_register, imm: Const12::new(0)});
        self.push_instruction_no_compress(RiscvInstr::SW { rs1: GLOBAL_BASE, rs2: new_size_register, imm: Const12::new(0)});

        // Otherwise, the result is already set to -1 (failure) and we skip the success block.
        // Value is stored on the runtime stack for the caller to use.
        let offset = self.get_runtime_stack_offset_from_last_vb().add(4);
        let stack_offset = self.stack_logical_offset_to_imm(offset);
        self.push_instruction(RiscvInstr::SW { rs1: STACK_POINTER, rs2: result_register, imm: Const12::new(stack_offset) });
        self.add_atomic_vb(AtomicVB::Resolved{val_type: ValueType::I32, offset});
    }

    fn visit_i32_const(&mut self,value:i32) {
        self.add_atomic_vb(AtomicVB::I32Const { imm: value });
    }

    fn visit_i64_const(&mut self,value:i64) {
        self.add_atomic_vb(AtomicVB::I64Const { imm: value });
    }

    fn visit_f32_const(&mut self,value:Ieee32) {
        self.add_atomic_vb(AtomicVB::F32Const { imm: value.bits() });
    }

    fn visit_f64_const(&mut self,value:Ieee64) {
        self.add_atomic_vb(AtomicVB::F64Const { imm: value.bits() });
    }

    fn visit_i32_eqz(&mut self) {
        self.add_unary_vb(UnaryVB::I32EqZ);
    }

    fn visit_i32_eq(&mut self) {
        self.add_binary_vb(BinaryVB::I32Eq);
    }

    fn visit_i32_ne(&mut self) {
        self.add_binary_vb(BinaryVB::I32Ne);
    }

    fn visit_i32_lt_s(&mut self) {
        self.add_binary_vb(BinaryVB::I32LtS);
    }

    fn visit_i32_lt_u(&mut self) {
        self.add_binary_vb(BinaryVB::I32LtU);
    }

    fn visit_i32_gt_s(&mut self) {
        self.add_binary_vb(BinaryVB::I32GtS);
    }

    fn visit_i32_gt_u(&mut self) {
        self.add_binary_vb(BinaryVB::I32GtU);
    }

    fn visit_i32_le_s(&mut self) {
        self.add_binary_vb(BinaryVB::I32LeS);
    }

    fn visit_i32_le_u(&mut self) {
        self.add_binary_vb(BinaryVB::I32LeU);
    }

    fn visit_i32_ge_s(&mut self) {
        self.add_binary_vb(BinaryVB::I32GeS);
    }

    fn visit_i32_ge_u(&mut self) {
        self.add_binary_vb(BinaryVB::I32GeU);
    } 
    
    fn visit_i64_eqz(&mut self) {
        self.add_unary_vb(UnaryVB::I64EqZ);
    }

    fn visit_i64_eq(&mut self) {
        self.add_binary_vb(BinaryVB::I64Eq);
    }

    fn visit_i64_ne(&mut self) {
        self.add_binary_vb(BinaryVB::I64Ne);
    }

    fn visit_i64_lt_s(&mut self) {
        self.add_binary_vb(BinaryVB::I64LtS);
    }

    fn visit_i64_lt_u(&mut self) {
        self.add_binary_vb(BinaryVB::I64LtU);
    }

    fn visit_i64_gt_s(&mut self) {
        self.add_binary_vb(BinaryVB::I64GtS);
    }

    fn visit_i64_gt_u(&mut self) {
        self.add_binary_vb(BinaryVB::I64GtU);
    }

    fn visit_i64_le_s(&mut self) {
        self.add_binary_vb(BinaryVB::I64LeS);
    }

    fn visit_i64_le_u(&mut self) {
        self.add_binary_vb(BinaryVB::I64LeU);
    }

    fn visit_i64_ge_s(&mut self) {
        self.add_binary_vb(BinaryVB::I64GeS);
    }

    fn visit_i64_ge_u(&mut self) {
        self.add_binary_vb(BinaryVB::I64GeU);
    }

    fn visit_f32_eq(&mut self) {
        self.add_binary_vb(BinaryVB::F32Eq);
    }

    fn visit_f32_ne(&mut self) {
        self.add_binary_vb(BinaryVB::F32Ne);
    }

    fn visit_f32_lt(&mut self) {
        self.add_binary_vb(BinaryVB::F32Lt);
    }

    fn visit_f32_gt(&mut self) {
        self.add_binary_vb(BinaryVB::F32Gt);
    }

    fn visit_f32_le(&mut self) {
        self.add_binary_vb(BinaryVB::F32Le);
    }

    fn visit_f32_ge(&mut self) {
        self.add_binary_vb(BinaryVB::F32Ge);
    }

    fn visit_f64_eq(&mut self) {
        self.add_binary_vb(BinaryVB::F64Eq);
    }

    fn visit_f64_ne(&mut self) {
        self.add_binary_vb(BinaryVB::F64Ne);
    }

    fn visit_f64_lt(&mut self) {
        self.add_binary_vb(BinaryVB::F64Lt);
    }

    fn visit_f64_gt(&mut self) {
        self.add_binary_vb(BinaryVB::F64Gt);
    }

    fn visit_f64_le(&mut self) {
        self.add_binary_vb(BinaryVB::F64Le);
    }

    fn visit_f64_ge(&mut self) {
        self.add_binary_vb(BinaryVB::F64Ge);
    }

    fn visit_i32_clz(&mut self) {
        self.add_unary_vb(UnaryVB::I32Clz);
    }

    fn visit_i32_ctz(&mut self) {
        self.add_unary_vb(UnaryVB::I32Ctz);
    }

    fn visit_i32_popcnt(&mut self) {
        self.add_unary_vb(UnaryVB::I32PopCnt);
    }

    fn visit_i32_add(&mut self) {
        self.add_binary_vb(BinaryVB::I32Add);
    }

    fn visit_i32_sub(&mut self) {
        self.add_binary_vb(BinaryVB::I32Sub);
    }

    fn visit_i32_mul(&mut self) {
        self.add_binary_vb(BinaryVB::I32Mul);
    }

    fn visit_i32_div_s(&mut self) {
        self.add_binary_vb(BinaryVB::I32DivS);
    }

    fn visit_i32_div_u(&mut self) {
        self.add_binary_vb(BinaryVB::I32DivU);
    }

    fn visit_i32_rem_s(&mut self) {
        self.add_binary_vb(BinaryVB::I32RemS);
    }

    fn visit_i32_rem_u(&mut self) {
        self.add_binary_vb(BinaryVB::I32RemU);
    }

    fn visit_i32_and(&mut self) {
        self.add_binary_vb(BinaryVB::I32And);
    }

    fn visit_i32_or(&mut self) {
        self.add_binary_vb(BinaryVB::I32Or);
    }

    fn visit_i32_xor(&mut self) {
        self.add_binary_vb(BinaryVB::I32Xor);
    }

    fn visit_i32_shl(&mut self) {
        self.add_binary_vb(BinaryVB::I32Shl);
    }

    fn visit_i32_shr_s(&mut self) {
        self.add_binary_vb(BinaryVB::I32ShrS);
    }

    fn visit_i32_shr_u(&mut self) {
        self.add_binary_vb(BinaryVB::I32ShrU);
    }

    fn visit_i32_rotl(&mut self) {
        self.add_binary_vb(BinaryVB::I32Rotl);
    }

    fn visit_i32_rotr(&mut self) {
        self.add_binary_vb(BinaryVB::I32Rotr);
    }

    fn visit_i64_clz(&mut self) {
        self.add_unary_vb(UnaryVB::I64Clz);
    }

    fn visit_i64_ctz(&mut self) {
        self.add_unary_vb(UnaryVB::I64Ctz);
    }

    fn visit_i64_popcnt(&mut self) {
        self.add_unary_vb(UnaryVB::I64PopCnt);
    }

    fn visit_i64_add(&mut self) {
        self.add_binary_vb(BinaryVB::I64Add);
    }

    fn visit_i64_sub(&mut self) {
        self.add_binary_vb(BinaryVB::I64Sub);
    }

    fn visit_i64_mul(&mut self) {
        self.add_binary_vb(BinaryVB::I64Mul);
    }

    fn visit_i64_div_s(&mut self) {
        self.add_binary_vb(BinaryVB::I64DivS);
    }

    fn visit_i64_div_u(&mut self) {
        self.add_binary_vb(BinaryVB::I64DivU);
    }

    fn visit_i64_rem_s(&mut self) {
        self.add_binary_vb(BinaryVB::I64RemS);
    }

    fn visit_i64_rem_u(&mut self) {
        self.add_binary_vb(BinaryVB::I64RemU);
    }

    fn visit_i64_and(&mut self) {
        self.add_binary_vb(BinaryVB::I64And);
    }

    fn visit_i64_or(&mut self) {
        self.add_binary_vb(BinaryVB::I64Or);
    }

    fn visit_i64_xor(&mut self) {
        self.add_binary_vb(BinaryVB::I64Xor);
    }

    fn visit_i64_shl(&mut self) {
        self.add_binary_vb(BinaryVB::I64Shl);
    }

    fn visit_i64_shr_s(&mut self) {
        self.add_binary_vb(BinaryVB::I64ShrS);
    }

    fn visit_i64_shr_u(&mut self) {
        self.add_binary_vb(BinaryVB::I64ShrU);
    }

    fn visit_i64_rotl(&mut self) {
        self.add_binary_vb(BinaryVB::I64Rotl);
    }

    fn visit_i64_rotr(&mut self) {
        self.add_binary_vb(BinaryVB::I64Rotr);
    }

    fn visit_f32_abs(&mut self) {
       self.add_unary_vb(UnaryVB::F32Abs);
    }

    fn visit_f32_neg(&mut self) {
        self.add_unary_vb(UnaryVB::F32Neg);
    }

    fn visit_f32_ceil(&mut self) {
        self.add_unary_vb(UnaryVB::F32Ceil);
    }

    fn visit_f32_floor(&mut self) {
        self.add_unary_vb(UnaryVB::F32Floor);
    }

    fn visit_f32_trunc(&mut self) {
        self.add_unary_vb(UnaryVB::F32Trunc);
    }

    fn visit_f32_nearest(&mut self) {
        self.add_unary_vb(UnaryVB::F32Nearest);
    }

    fn visit_f32_sqrt(&mut self) {
        self.add_unary_vb(UnaryVB::F32Sqrt);
    }

    fn visit_f32_add(&mut self) {
        self.add_binary_vb(BinaryVB::F32Add);
    }

    fn visit_f32_sub(&mut self) {
        self.add_binary_vb(BinaryVB::F32Sub);
    }

    fn visit_f32_mul(&mut self) {
        self.add_binary_vb(BinaryVB::F32Mul);
    }

    fn visit_f32_div(&mut self) {
        self.add_binary_vb(BinaryVB::F32Div);
    }

    fn visit_f32_min(&mut self) {
        self.add_binary_vb(BinaryVB::F32Min);
    }

    fn visit_f32_max(&mut self) {
        self.add_binary_vb(BinaryVB::F32Max);
    }

    fn visit_f32_copysign(&mut self) {
        self.add_binary_vb(BinaryVB::F32CopySign);
    }

    fn visit_f64_abs(&mut self) {
        self.add_unary_vb(UnaryVB::F64Abs);
    }

    fn visit_f64_neg(&mut self) {
        self.add_unary_vb(UnaryVB::F64Neg);
    }

    fn visit_f64_ceil(&mut self) {
        self.add_unary_vb(UnaryVB::F64Ceil);
    }

    fn visit_f64_floor(&mut self) {
        self.add_unary_vb(UnaryVB::F64Floor);
    }

    fn visit_f64_trunc(&mut self) {
        self.add_unary_vb(UnaryVB::F64Trunc);
    }

    fn visit_f64_nearest(&mut self) {
        self.add_unary_vb(UnaryVB::F64Nearest);
    }

    fn visit_f64_sqrt(&mut self) {
        self.add_unary_vb(UnaryVB::F64Sqrt);
    }

    fn visit_f64_add(&mut self) {
        self.add_binary_vb(BinaryVB::F64Add);
    }

    fn visit_f64_sub(&mut self) {
        self.add_binary_vb(BinaryVB::F64Sub);
    }

    fn visit_f64_mul(&mut self) {
        self.add_binary_vb(BinaryVB::F64Mul);
    }

    fn visit_f64_div(&mut self) {
        self.add_binary_vb(BinaryVB::F64Div);
    }

    fn visit_f64_min(&mut self) {
        self.add_binary_vb(BinaryVB::F64Min);
    }

    fn visit_f64_max(&mut self) {
        self.add_binary_vb(BinaryVB::F64Max);
    }

    fn visit_f64_copysign(&mut self) {
        self.add_binary_vb(BinaryVB::F64CopySign);
    }

    fn visit_i32_wrap_i64(&mut self) {
        self.add_unary_vb(UnaryVB::I32WrapI64);
    }

    fn visit_i32_trunc_f32_s(&mut self) {
        self.add_unary_vb(UnaryVB::I32TruncF32S);
    }

    fn visit_i32_trunc_f32_u(&mut self) {
        self.add_unary_vb(UnaryVB::I32TruncF32U);
    }

    fn visit_i32_trunc_f64_s(&mut self) {
        self.add_unary_vb(UnaryVB::I32TruncF64S);
    }

    fn visit_i32_trunc_f64_u(&mut self) {
        self.add_unary_vb(UnaryVB::I32TruncF64U);
    }

    fn visit_i64_extend_i32_s(&mut self) {
        self.add_unary_vb(UnaryVB::I64ExtendI32S);
    }

    fn visit_i64_extend_i32_u(&mut self) {
        self.add_unary_vb(UnaryVB::I64ExtendI32U);
    }

    fn visit_i64_trunc_f32_s(&mut self) {
        self.add_unary_vb(UnaryVB::I64TruncF32S);
    }

    fn visit_i64_trunc_f32_u(&mut self) {
        self.add_unary_vb(UnaryVB::I64TruncF32U);
    }

    fn visit_i64_trunc_f64_s(&mut self) {
        self.add_unary_vb(UnaryVB::I64TruncF64S);
    }

    fn visit_i64_trunc_f64_u(&mut self) {
        self.add_unary_vb(UnaryVB::I64TruncF64U);
    }

    fn visit_f32_convert_i32_s(&mut self) {
        self.add_unary_vb(UnaryVB::F32ConvertI32S);
    }

    fn visit_f32_convert_i32_u(&mut self) {
        self.add_unary_vb(UnaryVB::F32ConvertI32U);
    }

    fn visit_f32_convert_i64_s(&mut self) {
        self.add_unary_vb(UnaryVB::F32ConvertI64S);
    }

    fn visit_f32_convert_i64_u(&mut self) {
        self.add_unary_vb(UnaryVB::F32ConvertI64U);
    }

    fn visit_f32_demote_f64(&mut self) {
        self.add_unary_vb(UnaryVB::F32DemoteF64);
    }

    fn visit_f64_convert_i32_s(&mut self) {
        self.add_unary_vb(UnaryVB::F64ConvertI32S);
    }

    fn visit_f64_convert_i32_u(&mut self) {
        self.add_unary_vb(UnaryVB::F64ConvertI32U);
    }

    fn visit_f64_convert_i64_s(&mut self) {
        self.add_unary_vb(UnaryVB::F64ConvertI64S);
    }

    fn visit_f64_convert_i64_u(&mut self) {
        self.add_unary_vb(UnaryVB::F64ConvertI64U);
    }

    fn visit_f64_promote_f32(&mut self) {
        self.add_unary_vb(UnaryVB::F64PromoteF32);
    }

    fn visit_i32_reinterpret_f32(&mut self) {
        self.add_unary_vb(UnaryVB::I32ReinterpretF32);
    }

    fn visit_i64_reinterpret_f64(&mut self) {
        self.add_unary_vb(UnaryVB::I64ReinterpretF64);
    }

    fn visit_f32_reinterpret_i32(&mut self) {
        self.add_unary_vb(UnaryVB::F32ReinterpretI32);
    }

    fn visit_f64_reinterpret_i64(&mut self) {
        self.add_unary_vb(UnaryVB::F64ReinterpretI64);
    }
}

fn val_type_size(ty: &ValType) -> ValueSize {
    match ty{
        ValType::I32 | ValType::F32 => ValueSize::Word,
        ValType::I64 | ValType::F64 => ValueSize::DoubleWord,
        ValType::V128 => panic!("V128 not supported"),
        ValType::Ref(..) => panic!("Ref not supported")
    }
}
