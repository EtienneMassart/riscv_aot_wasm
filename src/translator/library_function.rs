// use env_wrapper::wrap_env;

use alloc::vec;
use alloc::vec::Vec;
use defmt::Format;
use crate::isa_model::{Const12, Const20, GPRegister, RegisterPair, FPRegister, MapperLocation, ValueSize, ValueType};
use crate::isa_model::RiscvInstr;
use crate::parse_and_translate::WasmRuntime;
use crate::translator::Translator;
use wasmparser::SubType;

#[derive(Debug, Clone, Format)]
pub(crate) enum LibraryFunction {
    F32Ceil,
    F32Floor,
    F32Trunc,
    F32Nearest,
    F64Ceil,
    F64Floor,
    F64Trunc,
    F64Nearest,
    I64TruncF32S,
    I64TruncF32U,
    I64TruncF64S,
    I64TruncF64U,
    F32ConvertI64S,
    F32ConvertI64U,
    F64ConvertI64S,
    F64ConvertI64U,
    F32Min,
    F32Max,
    F64Min,
    F64Max,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    I64Shl,
    I64ShrS,
    I64ShrU,
    I64Rotl,
    I64Rotr,
}

impl LibraryFunction {
    fn argument_type(&self) -> ValueType {
        match self {
            LibraryFunction::F32Ceil
            | LibraryFunction::F32Floor
            | LibraryFunction::F32Trunc
            | LibraryFunction::F32Nearest
            | LibraryFunction::I64TruncF32S
            | LibraryFunction::I64TruncF32U
            | LibraryFunction::F32Min
            | LibraryFunction::F32Max => ValueType::F32,
            LibraryFunction::F64Ceil
            | LibraryFunction::F64Floor
            | LibraryFunction::F64Trunc
            | LibraryFunction::F64Nearest
            | LibraryFunction::I64TruncF64S
            | LibraryFunction::I64TruncF64U
            | LibraryFunction::F64Min
            | LibraryFunction::F64Max => ValueType::F64,
            LibraryFunction::F32ConvertI64S
            | LibraryFunction::F32ConvertI64U
            | LibraryFunction::F64ConvertI64S
            | LibraryFunction::F64ConvertI64U
            | LibraryFunction::I64DivS
            | LibraryFunction::I64DivU
            | LibraryFunction::I64RemS
            | LibraryFunction::I64RemU
            | LibraryFunction::I64Shl
            | LibraryFunction::I64ShrS
            | LibraryFunction::I64ShrU
            | LibraryFunction::I64Rotl
            | LibraryFunction::I64Rotr => ValueType::I64,
        }
    }

    fn clobbered_fp_registers(&self, _has_second_arg: bool, _result_type: ValueType) -> Vec<FPRegister> {
        let mut fp_clobbered = Vec::new();

        // Helper ABI input registers (x27/x28/x29/x30, f28/f29) are scratch for argument passing,
        // so they are intentionally not preserved here.
        //
        // For naked helpers, clobbers are declared explicitly from the assembly body.
        // For non-naked runtime helpers, keep a small return-register fallback until each
        // helper is audited and made explicit.
        match self {
            LibraryFunction::F32Ceil
            | LibraryFunction::F32Floor
            | LibraryFunction::F32Trunc
            | LibraryFunction::F32Nearest
            | LibraryFunction::F64Ceil
            | LibraryFunction::F64Floor
            | LibraryFunction::F64Trunc
            | LibraryFunction::F64Nearest => {
                fp_clobbered.push(FPRegister(10));
                fp_clobbered.push(FPRegister(11));
                fp_clobbered.push(FPRegister(12));
            }
            LibraryFunction::F32Min
            | LibraryFunction::F32Max
            | LibraryFunction::F64Min
            | LibraryFunction::F64Max => {
                fp_clobbered.push(FPRegister(10));
            }
            LibraryFunction::F32ConvertI64S | LibraryFunction::F32ConvertI64U => {
                fp_clobbered.push(FPRegister(10));
                fp_clobbered.push(FPRegister(11));
                fp_clobbered.push(FPRegister(12));
            }
            LibraryFunction::F64ConvertI64S | LibraryFunction::F64ConvertI64U => {
                fp_clobbered.push(FPRegister(10));
                fp_clobbered.push(FPRegister(11));
                fp_clobbered.push(FPRegister(12));
            }
            LibraryFunction::I64Shl
            | LibraryFunction::I64ShrS
            | LibraryFunction::I64ShrU
            | LibraryFunction::I64Rotl
            | LibraryFunction::I64Rotr
            | LibraryFunction::I64TruncF32S
            | LibraryFunction::I64TruncF32U
            | LibraryFunction::I64TruncF64S
            | LibraryFunction::I64TruncF64U
            | LibraryFunction::I64DivS
            | LibraryFunction::I64DivU
            | LibraryFunction::I64RemS
            | LibraryFunction::I64RemU => {}
        }

        fp_clobbered
    }
}

pub(crate) struct Operands<'a> {
    pub first: &'a MapperLocation,
    pub second: Option<&'a MapperLocation>,
}

impl<'a> Operands<'a> {
    pub(crate) fn new_single(first: &MapperLocation) -> Operands<'_> {
        Operands {
            first,
            second: None,
        }
    }

    pub(crate) fn new_double(first: &'a MapperLocation, second: &'a MapperLocation) -> Operands<'a> {
        Operands {
            first,
            second: Some(second),
        }
    }
}

impl Translator<'_,'_>{

    /// Save a broad GP set (including ra and s-registers) around external helper calls.
    /// Uses Zcmp compressed prologue/epilogue instructions for code-size efficiency.
    fn push_external_gp_preserve_frame(&mut self) {
        // rlist=15 => {ra, s0-s11}, spimm=0 => stack adjustment 64 bytes on RV32I.
        self.push_instruction(RiscvInstr::CMPUSH { rlist: 15, spimm: 0 });
    }

    /// Restore GP registers previously saved by `push_external_gp_preserve_frame`.
    fn pop_external_gp_preserve_frame(&mut self) {
        self.push_instruction(RiscvInstr::CMPOP { rlist: 15, spimm: 0 });
    }

    fn preserve_call_clobbered_fp_values(
        &mut self,
        fp_clobbered: &[FPRegister],
        scratch_variable_map: &mut Vec<MapperLocation>,
    ) {
        let mut preserve_indices = Vec::new();
        for (i, location) in scratch_variable_map.iter().enumerate() {
            let needs_preserve = match location {
                MapperLocation::FPRegister { reg, .. } => fp_clobbered.contains(reg),
                _ => false,
            };

            if needs_preserve {
                preserve_indices.push(i);
            }
        }

        for i in preserve_indices {
            let location = scratch_variable_map[i].clone();

            let used_registers = scratch_variable_map.clone();
            let target = match location.get_type() {
                ValueType::F32 => MapperLocation::FPRegister {
                    reg: self.next_available_fp_register(scratch_variable_map, &used_registers),
                    val_size: ValueSize::Word,
                },
                ValueType::F64 => MapperLocation::FPRegister {
                    reg: self.next_available_fp_register(scratch_variable_map, &used_registers),
                    val_size: ValueSize::DoubleWord,
                },
                _ => panic!("Unexpected type for FP register preservation: {:?}", location.get_type()),
            };

            let remapped = location.map_to_location(&target, self, scratch_variable_map, &used_registers);
            scratch_variable_map[i] = remapped;
        }
    }

    /// generates machine code for calling a runtime function from the translated wasm functions. This is to be used in code generation for subroutines that are not supported through a short sequence of machine instructions.
    /// It is assumed that the runtime function takes at most 2 arguments from the same size (32-bit word or 64-bit double-word) and returns one result.
    /// 
    /// ### Parameters:
    /// - **target**: the result is moved to the target location if available, otherwise placed in a scratch register
    /// - **function**: runtime function to be called
    /// - **ops**: Operands struct containing one or two operands
    /// - **op_size**: size of the operand(s)
    /// - **result_type**: type of the result
    /// - **scratch_variable_map**: needs to be passed down in order to allocate a free register for the result, in case no target location is specified.
    /// 
    /// ### Return value:
    ///  location of the result value
    pub(crate) fn call_library_function(&mut self, target: Option<&MapperLocation>, function:LibraryFunction, ops: Operands<'_>, _op_size: ValueSize, result_type: ValueType, scratch_variable_map : &mut Vec<MapperLocation>) -> MapperLocation {
        let fp_clobbered = function.clobbered_fp_registers(ops.second.is_some(), result_type);
        self.setup_ops(function.clone(), ops, scratch_variable_map);
        self.preserve_call_clobbered_fp_values(&fp_clobbered, scratch_variable_map);

        self.push_external_gp_preserve_frame();
        self.perform_external_call(function);
        self.pop_external_gp_preserve_frame();
        self.process_external_result(target, result_type, scratch_variable_map)
    }

    /// Loads up to two arguments according to the custom helper ABI.
    ///
    /// Integer arguments:
    /// - i32: x27, x28
    /// - i64: x27:x28, x29:x30
    /// Floating-point arguments:
    /// - f32: f28, f29
    /// - f64: f28, f29
    fn setup_ops(&mut self, function: LibraryFunction, args: Operands<'_>, scratch_variable_map: &mut Vec<MapperLocation>) {
        let arg_type = function.argument_type();

        if args.second.is_none() {
            let target_0 = match arg_type {
                ValueType::I32 => MapperLocation::GPRegister(GPRegister(27)),
                ValueType::I64 => MapperLocation::RegisterPair(RegisterPair::new(27, 28)),
                ValueType::F32 => MapperLocation::FPRegister { reg: FPRegister(28), val_size: ValueSize::Word },
                ValueType::F64 => MapperLocation::FPRegister { reg: FPRegister(28), val_size: ValueSize::DoubleWord },
            };
            let _ = args.first.map_to_location(&target_0, self, scratch_variable_map, &vec![]);
            return;
        }

        let (target_0, target_1) = match arg_type {
            ValueType::I32 => (
                MapperLocation::GPRegister(GPRegister(27)),
                MapperLocation::GPRegister(GPRegister(28)),
            ),
            ValueType::I64 => (
                MapperLocation::RegisterPair(RegisterPair::new(27, 28)),
                MapperLocation::RegisterPair(RegisterPair::new(29, 30)),
            ),
            ValueType::F32 => (
                MapperLocation::FPRegister { reg: FPRegister(28), val_size: ValueSize::Word },
                MapperLocation::FPRegister { reg: FPRegister(29), val_size: ValueSize::Word },
            ),
            ValueType::F64 => (
                MapperLocation::FPRegister { reg: FPRegister(28), val_size: ValueSize::DoubleWord },
                MapperLocation::FPRegister { reg: FPRegister(29), val_size: ValueSize::DoubleWord },
            ),
        };

        let second = args.second.unwrap();
        if args.first == &target_1 && second == &target_0 {
            // Swap via temporary location to avoid clobbering when both inputs are already in target regs.
            let used_registers = vec![target_0.clone(), target_1.clone(), args.first.clone(), second.clone()];
            let temp = match arg_type {
                ValueType::I32 => MapperLocation::GPRegister(self.next_available_gp_register(scratch_variable_map, &used_registers)),
                ValueType::I64 => MapperLocation::RegisterPair(self.next_available_register_pair(scratch_variable_map, &used_registers)),
                ValueType::F32 => MapperLocation::FPRegister {
                    reg: self.next_available_fp_register(scratch_variable_map, &used_registers),
                    val_size: ValueSize::Word,
                },
                ValueType::F64 => MapperLocation::FPRegister {
                    reg: self.next_available_fp_register(scratch_variable_map, &used_registers),
                    val_size: ValueSize::DoubleWord,
                },
            };
            let _ = args.first.map_to_location(&temp, self, scratch_variable_map, &vec![]);
            let _ = second.map_to_location(&target_1, self, scratch_variable_map, &vec![]);
            let _ = temp.map_to_location(&target_0, self, scratch_variable_map, &vec![]);
        } else if second == &target_0 {
            // If arg1 occupies arg0 target, place arg1 first to avoid overwriting it.
            let _ = second.map_to_location(&target_1, self, scratch_variable_map, &vec![]);
            let _ = args.first.map_to_location(&target_0, self, scratch_variable_map, &vec![]);
        } else {
            let _ = args.first.map_to_location(&target_0, self, scratch_variable_map, &vec![]);
            let _ = second.map_to_location(&target_1, self, scratch_variable_map, &vec![]);
        }
    }

    /// Helper method that maps the function to be called to its address and emits an
    /// indirect RISC-V call. The absolute address is materialized with LUI/ADDI and
    /// then called via JALR.
    fn perform_external_call(&mut self, function: LibraryFunction) {
        let library_function_ptr = match function {
            LibraryFunction::F32Ceil => WasmRuntime::f32_ceil as u32,
            LibraryFunction::F32Floor => WasmRuntime::f32_floor as u32,
            LibraryFunction::F32Trunc => WasmRuntime::f32_trunc as u32,
            LibraryFunction::F32Nearest => WasmRuntime::f32_nearest as u32,
            LibraryFunction::F64Ceil => WasmRuntime::f64_ceil as u32,
            LibraryFunction::F64Floor => WasmRuntime::f64_floor as u32,
            LibraryFunction::F64Trunc => WasmRuntime::f64_trunc as u32,
            LibraryFunction::F64Nearest => WasmRuntime::f64_nearest as u32,
            LibraryFunction::I64TruncF32S => WasmRuntime::i64_trunc_f32_s as u32,
            LibraryFunction::I64TruncF32U => WasmRuntime::i64_trunc_f32_u as u32,
            LibraryFunction::I64TruncF64S => WasmRuntime::i64_trunc_f64_s as u32,
            LibraryFunction::I64TruncF64U => WasmRuntime::i64_trunc_f64_u as u32,
            LibraryFunction::F32ConvertI64S => WasmRuntime::f32_convert_i64_s as u32,
            LibraryFunction::F32ConvertI64U => WasmRuntime::f32_convert_i64_u as u32,
            LibraryFunction::F64ConvertI64S => WasmRuntime::f64_convert_i64_s as u32,
            LibraryFunction::F64ConvertI64U => WasmRuntime::f64_convert_i64_u as u32,
            LibraryFunction::F64Min => WasmRuntime::f64_min as u32,
            LibraryFunction::F64Max => WasmRuntime::f64_max as u32,
            LibraryFunction::I64DivS => WasmRuntime::i64_div_s as u32,
            LibraryFunction::I64DivU => WasmRuntime::i64_div_u as u32,
            LibraryFunction::I64RemS => WasmRuntime::i64_rem_s as u32,
            LibraryFunction::I64RemU => WasmRuntime::i64_rem_u as u32,
            LibraryFunction::F32Min => WasmRuntime::f32_min as u32,
            LibraryFunction::F32Max => WasmRuntime::f32_max as u32,
            LibraryFunction::I64Shl => WasmRuntime::i64_shl as u32,
            LibraryFunction::I64ShrS => WasmRuntime::i64_shr_s as u32,
            LibraryFunction::I64ShrU => WasmRuntime::i64_shr_u as u32,
            LibraryFunction::I64Rotl => WasmRuntime::i64_rotl as u32,
            LibraryFunction::I64Rotr => WasmRuntime::i64_rotr as u32,
        };
        // Compute PC of the call instruction we are about to emit.
        let current_pc = self.wasm_runtime.instructions.as_ptr() as i32
            + (self.wasm_runtime.instruction_hw_count as i32) * 2;
        let offset = library_function_ptr as i32 - current_pc;


        // Two-instruction PC-relative call via AUIPC + JALR.
        let call_target = GPRegister(16);
        let upper = (offset.wrapping_add(0x800)) >> 12;
        let lower = (offset & 0xFFF) as i16;
        self.push_instruction(RiscvInstr::AUIPC {
            rd: call_target,
            imm: Const20::new(upper),
        });
        self.push_instruction(RiscvInstr::JALR {
            rd: GPRegister(1),
            rs1: call_target,
            imm: Const12::new(lower),
        });
        
    }

        /// Generates machine code to obtain the result returned by the runtime function from
    /// the RISC-V return registers and moves it to the target location.
    /// and moves it to the target location if available, otherwise to an available scratch register.
    fn process_external_result(&mut self, target: Option<&MapperLocation>, result_type: ValueType, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let used_registers = vec![];
        let target = target.cloned().unwrap_or_else(|| match result_type {
            ValueType::I32 => MapperLocation::GPRegister(self.next_available_gp_register(scratch_variable_map, &used_registers)),
            ValueType::I64 => MapperLocation::RegisterPair(self.next_available_register_pair(scratch_variable_map, &used_registers)),
            ValueType::F32 => MapperLocation::FPRegister {
                reg: self.next_available_fp_register(scratch_variable_map, &used_registers),
                val_size: ValueSize::Word,
            },
            ValueType::F64 => MapperLocation::FPRegister {
                reg: self.next_available_fp_register(scratch_variable_map, &used_registers),
                val_size: ValueSize::DoubleWord,
            },
        });

        match result_type {
            ValueType::I32 => GPRegister(10).map_to_location(Some(&target), self, scratch_variable_map),
            ValueType::I64 => RegisterPair::new(10, 11).map_to_location(Some(&target), self, scratch_variable_map),
            ValueType::F32 => FPRegister(10).map_to_location(Some(&target), self, scratch_variable_map, ValueSize::Word),
            ValueType::F64 => FPRegister(10).map_to_location(Some(&target), self, scratch_variable_map, ValueSize::DoubleWord),
        }
    }

}

impl WasmRuntime<'_> {
    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f32_ceil(_x: f32) -> f32 {
        // Custom helper ABI: input in f28, output in f10.
        // Corner cases:
        // - NaN/Inf: propagate input
        // - signed zero: preserve sign
        // - |x| >= 2^31: keep input to avoid invalid int conversion
        //
        // Control-flow steps:
        // 1) Detect NaN/Inf/zero via fclass and fast-return unchanged input.
        // 2) Reject out-of-range magnitudes (|x| >= 2^31) and return unchanged.
        // 3) Convert with round-up mode (rup) through integer domain.
        // 4) If rounded value becomes 0, restore sign to produce +/-0.
        unsafe {
            core::arch::naked_asm!(
            // Step 1: NaN / Inf / +/-0 checks via fclass.s.
                "fclass.s x18, f28",
                "li x19, 921", // (1<<9)|(1<<8)|(1<<7)|(1<<4)|(1<<3)
                "and x20, x18, x19",
                "bne x20, x0, 3f",

                // Step 2: if abs(x) >= 2^31 keep input unchanged.
                "fsgnjx.s f11, f28, f28",
                "lui x19, 0x4f000", // 2^31 as f32 bits (0x4f000000)
                "fmv.w.x f12, x19",
                "flt.s x18, f11, f12",
                "beq x18, x0, 3f",

                "fcvt.w.s x18, f28, rup",
                "beq x18, x0, 2f",
                // Step 3: convert int result back to f32.
                "fcvt.s.w f10, x18, rne",
                "ret",

                // Step 4: for rounded-zero results, preserve input sign.
                "2:",
                "fmv.w.x f10, x0",
                "fsgnj.s f10, f10, f28",
                "ret",

                // Fast-return path for special/out-of-range inputs.
                "3:",
                "fsgnj.s f10, f28, f28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f32_floor(_x: f32) -> f32 {
        // Custom helper ABI: input in f28, output in f10.
        // Corner cases:
        // - NaN/Inf: propagate input
        // - signed zero: preserve sign
        // - |x| >= 2^31: keep input to avoid invalid int conversion
        //
        // Control-flow steps mirror f32_ceil, but with round-down mode (rdn).
        unsafe {
            core::arch::naked_asm!(
            // Step 1: NaN/Inf/zero guard.
                "fclass.s x18, f28",
                "li x19, 921", // (1<<9)|(1<<8)|(1<<7)|(1<<4)|(1<<3)
                "and x20, x18, x19",
                "bne x20, x0, 3f",

                // Step 2: out-of-range guard.
                "fsgnjx.s f11, f28, f28",
                "lui x19, 0x4f000", // 2^31 as f32 bits (0x4f000000)
                "fmv.w.x f12, x19",
                "flt.s x18, f11, f12",
                "beq x18, x0, 3f",

                // Step 3: floor via integer conversion.
                "fcvt.w.s x18, f28, rdn",
                "fcvt.s.w f10, x18, rne",
                "ret",

                // Fast-return path for special/out-of-range inputs.
                "3:",
                "fsgnj.s f10, f28, f28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f32_trunc(_x: f32) -> f32 {
        // Custom helper ABI: input in f28, output in f10.
        // Corner cases:
        // - NaN/Inf: propagate input
        // - signed zero: preserve sign
        // - |x| >= 2^31: keep input to avoid invalid int conversion
        //
        // Control-flow steps mirror f32_ceil/floor, but with round-toward-zero mode (rtz).
        unsafe {
            core::arch::naked_asm!(
            // Step 1: NaN/Inf/zero guard.
                "fclass.s x18, f28",
                "li x19, 921", // (1<<9)|(1<<8)|(1<<7)|(1<<4)|(1<<3)
                "and x20, x18, x19",
                "bne x20, x0, 3f",

                // Step 2: out-of-range guard.
                "fsgnjx.s f11, f28, f28",
                "lui x19, 0x4f000", // 2^31 as f32 bits (0x4f000000)
                "fmv.w.x f12, x19",
                "flt.s x18, f11, f12",
                "beq x18, x0, 3f",

                // Step 3: truncation via integer conversion.
                "fcvt.w.s x18, f28, rtz",
                "beq x18, x0, 2f",
                "fcvt.s.w f10, x18, rne",
                "ret",

                // Step 4: preserve signed zero when result rounds to 0.
                "2:",
                "fmv.w.x f10, x0",
                "fsgnj.s f10, f10, f28",
                "ret",

                // Fast-return path for special/out-of-range inputs.
                "3:",
                "fsgnj.s f10, f28, f28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f32_nearest(_x: f32) -> f32 {
        // Custom helper ABI: input in f28, output in f10.
        // Corner cases:
        // - NaN/Inf: propagate input
        // - signed zero: preserve sign
        // - |x| >= 2^31: keep input to avoid invalid int conversion
        //
        // Control-flow steps mirror other f32 rounding helpers, using round-to-nearest-even (rne).
        unsafe {
            core::arch::naked_asm!(
            // Step 1: NaN/Inf/zero guard.
                "fclass.s x18, f28",
                "li x19, 921", // (1<<9)|(1<<8)|(1<<7)|(1<<4)|(1<<3)
                "and x20, x18, x19",
                "bne x20, x0, 3f",

                // Step 2: out-of-range guard.
                "fsgnjx.s f11, f28, f28",
                "lui x19, 0x4f000", // 2^31 as f32 bits (0x4f000000)
                "fmv.w.x f12, x19",
                "flt.s x18, f11, f12",
                "beq x18, x0, 3f",

                // Step 3: nearest-even via integer conversion.
                "fcvt.w.s x18, f28, rne",
                "beq x18, x0, 2f",
                "fcvt.s.w f10, x18, rne",
                "ret",

                // Step 4: preserve signed zero when result rounds to 0.
                "2:",
                "fmv.w.x f10, x0",
                "fsgnj.s f10, f10, f28",
                "ret",

                // Fast-return path for special/out-of-range inputs.
                "3:",
                "fsgnj.s f10, f28, f28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f64_ceil(_x: f64) -> f64 {
        // Custom helper ABI: input in f28, output in f10.
        // Correct f64.ceil for full double-precision integer range.
        //
        // Control-flow steps:
        // 1) Decode sign/exponent and fast-return NaN/Inf/large integral values.
        // 2) Handle |x| < 1 separately (ceil is +1 for positive non-zero, signed zero otherwise).
        // 3) For 0 <= exp < 52, clear fractional bits in the IEEE-754 payload.
        // 4) If positive and fractional bits were dropped, add 1.0.
        unsafe {
            core::arch::naked_asm!(
                // Decode bits: x20=lo, x21=hi.
                "addi sp, sp, -8",
                "fsd f28, 0(sp)",
                "lw x20, 0(sp)",
                "lw x21, 4(sp)",
                "srli x18, x21, 31",       // sign bit
                "srli x22, x21, 20",       // exponent field
                "andi x22, x22, 2047",

                // NaN/Inf: return input unchanged.
                "li x23, 2047",
                "beq x22, x23, 9f",

                // exp == 0: zero or subnormal.
                "beq x22, x0, 1f",

                // unbiased exponent.
                "addi x22, x22, -1023",

                // |x| < 1.0 path.
                "blt x22, x0, 2f",

                // exp >= 52 => already integral.
                "li x23, 52",
                "bge x22, x23, 9f",

                // frac_bits = 52 - exp in x24 (1..52 here).
                "li x24, 52",
                "sub x24, x24, x22",
                "mv x25, x0",              // dropped_fraction flag

                "li x23, 32",
                "blt x23, x24, 3f",        // frac_bits > 32
                "beq x24, x23, 4f",        // frac_bits == 32

                // frac_bits < 32: clear low-word fraction bits.
                "li x26, 1",
                "sll x26, x26, x24",
                "addi x26, x26, -1",
                "and x27, x20, x26",
                "beq x27, x0, 5f",
                "li x25, 1",
                "5:",
                "xori x26, x26, -1",
                "and x20, x20, x26",
                "j 6f",

                // frac_bits == 32: whole low word is fractional.
                "4:",
                "beq x20, x0, 6f",
                "li x25, 1",
                "mv x20, x0",
                "j 6f",

                // frac_bits > 32: clear low word and low (frac_bits-32) mantissa bits in high word.
                "3:",
                "addi x26, x24, -32",
                "li x27, 1",
                "sll x27, x27, x26",
                "addi x27, x27, -1",
                "and x28, x21, x27",
                "or x28, x28, x20",
                "beq x28, x0, 31f",
                "li x25, 1",
                "31:",
                "xori x27, x27, -1",
                "and x21, x21, x27",
                "mv x20, x0",

                // Rebuild truncated value.
                "6:",
                "sw x20, 0(sp)",
                "sw x21, 4(sp)",
                "fld f10, 0(sp)",
                "addi sp, sp, 8",

                // No fractional part dropped -> already integral.
                "beq x25, x0, 8f",

                // Negative numbers: ceil equals trunc toward zero.
                "bne x18, x0, 8f",

                // Positive and had fractional bits: add 1.0.
                "li x19, 1",
                "fcvt.d.w f11, x19",
                "fadd.d f10, f10, f11",
                "8:",
                "ret",

                // exp == 0 (zero/subnormal)
                "1:",
                // +0.0 stays +0.0 via passthrough; -0.0 also passthrough.
                "slli x23, x21, 1",
                "or x23, x23, x20",
                "beq x23, x0, 9f",
                // Positive subnormal => ceil = +1.0.
                "bne x18, x0, 7f",
                "addi sp, sp, 8",
                "li x19, 1",
                "fcvt.d.w f10, x19",
                "ret",

                // Negative subnormal => ceil = -0.0.
                "7:",
                "addi sp, sp, 8",
                "fcvt.d.w f10, x0",
                "fsgnj.d f10, f10, f28",
                "ret",

                // 0 < |x| < 1.0
                "2:",
                // Positive => +1.0
                "bne x18, x0, 21f",
                "addi sp, sp, 8",
                "li x19, 1",
                "fcvt.d.w f10, x19",
                "ret",
                // Negative => -0.0
                "21:",
                "addi sp, sp, 8",
                "fcvt.d.w f10, x0",
                "fsgnj.d f10, f10, f28",
                "ret",

                // Fast-return path for NaN/Inf/already-integral values.
                "9:",
                "addi sp, sp, 8",
                "fsgnj.d f10, f28, f28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f64_floor(_x: f64) -> f64 {
        // Custom helper ABI: input in f28, output in f10.
        // Correct f64.floor for full double-precision integer range.
        //
        // Control-flow steps:
        // 1) Decode sign/exponent and fast-return NaN/Inf/large integral values.
        // 2) Handle |x| < 1 separately (floor is -1 for negative non-zero, signed zero otherwise).
        // 3) For 0 <= exp < 52, clear fractional bits in the IEEE-754 payload.
        // 4) If negative and fractional bits were dropped, subtract 1.0.
        unsafe {
            core::arch::naked_asm!(
                // Decode bits: x20=lo, x21=hi.
                "addi sp, sp, -8",
                "fsd f28, 0(sp)",
                "lw x20, 0(sp)",
                "lw x21, 4(sp)",
                "srli x18, x21, 31",       // sign bit
                "srli x22, x21, 20",       // exponent field
                "andi x22, x22, 2047",

                // NaN/Inf: return input unchanged.
                "li x23, 2047",
                "beq x22, x23, 9f",

                // exp == 0: zero or subnormal.
                "beq x22, x0, 1f",

                // unbiased exponent.
                "addi x22, x22, -1023",

                // |x| < 1.0 path.
                "blt x22, x0, 2f",

                // exp >= 52 => already integral.
                "li x23, 52",
                "bge x22, x23, 9f",

                // frac_bits = 52 - exp in x24 (1..52 here).
                "li x24, 52",
                "sub x24, x24, x22",
                "mv x25, x0",              // dropped_fraction flag

                "li x23, 32",
                "blt x23, x24, 3f",        // frac_bits > 32
                "beq x24, x23, 4f",        // frac_bits == 32

                // frac_bits < 32: clear low-word fraction bits.
                "li x26, 1",
                "sll x26, x26, x24",
                "addi x26, x26, -1",
                "and x27, x20, x26",
                "beq x27, x0, 5f",
                "li x25, 1",
                "5:",
                "xori x26, x26, -1",
                "and x20, x20, x26",
                "j 6f",

                // frac_bits == 32: whole low word is fractional.
                "4:",
                "beq x20, x0, 6f",
                "li x25, 1",
                "mv x20, x0",
                "j 6f",

                // frac_bits > 32: clear low word and low (frac_bits-32) mantissa bits in high word.
                "3:",
                "addi x26, x24, -32",
                "li x27, 1",
                "sll x27, x27, x26",
                "addi x27, x27, -1",
                "and x28, x21, x27",
                "or x28, x28, x20",
                "beq x28, x0, 31f",
                "li x25, 1",
                "31:",
                "xori x27, x27, -1",
                "and x21, x21, x27",
                "mv x20, x0",

                // Rebuild truncated value.
                "6:",
                "sw x20, 0(sp)",
                "sw x21, 4(sp)",
                "fld f10, 0(sp)",
                "addi sp, sp, 8",

                // No fractional part dropped -> already integral.
                "beq x25, x0, 8f",

                // Positive numbers: floor equals trunc toward zero.
                "beq x18, x0, 8f",

                // Negative and had fractional bits: subtract 1.0.
                "li x19, 1",
                "fcvt.d.w f11, x19",
                "fsub.d f10, f10, f11",
                "8:",
                "ret",

                // exp == 0 (zero/subnormal)
                "1:",
                // +0.0 and -0.0 stay unchanged via passthrough.
                "slli x23, x21, 1",
                "or x23, x23, x20",
                "beq x23, x0, 9f",
                // Positive subnormal => floor = +0.0.
                "beq x18, x0, 7f",
                // Negative subnormal => floor = -1.0.
                "addi sp, sp, 8",
                "li x19, -1",
                "fcvt.d.w f10, x19",
                "ret",
                "7:",
                "addi sp, sp, 8",
                "fcvt.d.w f10, x0",
                "ret",

                // 0 < |x| < 1.0
                "2:",
                // Positive => +0.0
                "beq x18, x0, 21f",
                // Negative => -1.0
                "addi sp, sp, 8",
                "li x19, -1",
                "fcvt.d.w f10, x19",
                "ret",
                "21:",
                "addi sp, sp, 8",
                "fcvt.d.w f10, x0",
                "ret",

                // Fast-return path for NaN/Inf/already-integral values.
                "9:",
                "addi sp, sp, 8",
                "fsgnj.d f10, f28, f28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f64_trunc(_x: f64) -> f64 {
        // Custom helper ABI: input in f28, output in f10.
        // Correct f64.trunc for full double-precision integer range.
        //
        // Control-flow steps:
        // 1) Decode sign/exponent and fast-return NaN/Inf/large integral values.
        // 2) Handle |x| < 1 by returning signed zero.
        // 3) For 0 <= exp < 52, clear fractional bits in IEEE-754 payload.
        unsafe {
            core::arch::naked_asm!(
                // Decode bits: x20=lo, x21=hi.
                "addi sp, sp, -8",
                "fsd f28, 0(sp)",
                "lw x20, 0(sp)",
                "lw x21, 4(sp)",
                "srli x18, x21, 31",       // sign bit
                "srli x22, x21, 20",       // exponent field
                "andi x22, x22, 2047",

                // NaN/Inf: return input unchanged.
                "li x23, 2047",
                "beq x22, x23, 7f",

                // exp == 0: zero/subnormal => signed zero.
                "beq x22, x0, 1f",

                // unbiased exponent.
                "addi x22, x22, -1023",

                // |x| < 1.0 => signed zero.
                "blt x22, x0, 1f",

                // exp >= 52 => already integral.
                "li x23, 52",
                "bge x22, x23, 7f",

                // frac_bits = 52 - exp in x24 (1..52 here).
                "li x24, 52",
                "sub x24, x24, x22",

                "li x23, 32",
                "blt x23, x24, 2f",        // frac_bits > 32
                "beq x24, x23, 3f",        // frac_bits == 32

                // frac_bits < 32: clear low-word fraction bits.
                "li x26, 1",
                "sll x26, x26, x24",
                "addi x26, x26, -1",
                "xori x26, x26, -1",
                "and x20, x20, x26",
                "j 4f",

                // frac_bits == 32: clear full low word.
                "3:",
                "mv x20, x0",
                "j 4f",

                // frac_bits > 32: clear low word and low (frac_bits-32) mantissa bits in high word.
                "2:",
                "addi x26, x24, -32",
                "li x27, 1",
                "sll x27, x27, x26",
                "addi x27, x27, -1",
                "xori x27, x27, -1",
                "and x21, x21, x27",
                "mv x20, x0",

                // Rebuild truncated value.
                "4:",
                "sw x20, 0(sp)",
                "sw x21, 4(sp)",
                "fld f10, 0(sp)",
                "addi sp, sp, 8",
                "ret",

                // |x| < 1.0 or subnormal: return signed zero.
                "1:",
                "addi sp, sp, 8",
                "fcvt.d.w f10, x0",
                "fsgnj.d f10, f10, f28",
                "ret",

                // Fast-return path for NaN/Inf/already-integral values.
                "7:",
                "addi sp, sp, 8",
                "fsgnj.d f10, f28, f28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f64_nearest(_x: f64) -> f64 {
        // Custom helper ABI: input in f28, output in f10.
        // Corner cases:
        // - NaN/Inf: propagate input unchanged
        // - |x| >= 2^52: already integral in binary64, return as-is
        // - zero results preserve the operand sign
        //
        // Control-flow steps:
        // 1) Decode exponent and fast-return NaN/Inf/already-integral magnitudes (|x| >= 2^52).
        // 2) Build sign-aware +/-2^52 constant.
        // 3) Compute nearest-even by (x + magic) - magic with explicit rne rounding.
        // 4) If the rounded result is zero, restore the input sign.
        unsafe {
            core::arch::naked_asm!(
                // Step 1: decode exponent from raw bits.
                "addi sp, sp, -8",
                "fsd f28, 0(sp)",
                "lw x20, 0(sp)",
                "lw x21, 4(sp)",
                "srli x22, x21, 20",
                "andi x22, x22, 2047",

                // NaN/Inf passthrough.
                "li x23, 2047",
                "beq x22, x23, 7f",

                // |x| >= 2^52 is already integral in f64.
                "li x23, 1075",
                "bge x22, x23, 7f",

                // Step 2: build sign-aware magic = copysign(2^52, x).
                "lui x24, 0x43300",       // +2^52 high word
                "srli x18, x21, 31",      // sign bit
                "beq x18, x0, 1f",
                "lui x25, 0x80000",
                "or x24, x24, x25",
                "1:",
                "sw x0, 0(sp)",
                "sw x24, 4(sp)",
                "fld f11, 0(sp)",

                // Step 3: nearest-even round via add/sub with explicit rne.
                "fadd.d f10, f28, f11, rne",
                "fsub.d f10, f10, f11, rne",

                // Step 4: preserve the operand sign when the rounded result is zero.
                "fsd f10, 0(sp)",
                "lw x22, 0(sp)",
                "lw x23, 4(sp)",
                "slli x23, x23, 1",
                "or x23, x23, x22",
                "bne x23, x0, 2f",
                "fsgnj.d f10, f10, f28",
                "2:",
                "addi sp, sp, 8",
                "ret",

                // Fast-return path for NaN/Inf/already-integral values.
                "7:",
                "addi sp, sp, 8",
                "fsgnj.d f10, f28, f28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_trunc_f32_s(_x: f32) -> i64 {
        // Control-flow steps:
        // 1) Decode sign/exponent and reject NaN/Inf.
        // 2) Handle underflow to 0 and exact INT64_MIN boundary case.
        // 3) Build integer significand and shift according to unbiased exponent.
        // 4) Apply sign and return, otherwise trap on overflow/invalid.
        unsafe {
            core::arch::naked_asm!(
                // Step 1: load f32 bits and extract sign/exponent.
                "fmv.x.w x20, f28",
                "srli x18, x20, 31",
                "srli x21, x20, 23",
                "andi x21, x21, 255",
                "li x22, 255",
                "beq x21, x22, 7f",
                "addi x21, x21, -127",
                // Step 2a: |x| < 1.0 -> 0.
                "blt x21, x0, 6f",

                // Step 2b: exp == 63 is only valid for -2^63 exactly.
                "li x22, 63",
                "bne x21, x22, 1f",
                "beq x18, x0, 7f",
                "lui x22, 0x800",
                "addi x22, x22, -1",
                "and x22, x20, x22",
                "bne x22, x0, 7f",
                "mv x10, x0",
                "lui x11, 0x80000",
                "ret",

                "1:",
                // Step 3: construct integer magnitude from significand/exponent.
                "li x22, 63",
                "blt x22, x21, 7f",
                "lui x22, 0x800",
                "addi x22, x22, -1",
                "and x22, x20, x22",
                "lui x23, 0x800",
                "or x22, x22, x23",
                "addi x24, x21, -23",
                "blt x24, x0, 2f",
                "li x23, 32",
                "blt x24, x23, 3f",
                "addi x24, x24, -32",
                "sll x11, x22, x24",
                "mv x10, x0",
                "j 4f",

                "2:",
                "sub x24, x0, x24",
                "srl x10, x22, x24",
                "mv x11, x0",
                "j 4f",

                "3:",
                "sll x10, x22, x24",
                "li x23, 32",
                "sub x23, x23, x24",
                "srl x11, x22, x23",

                "4:",
                // Step 4: apply sign and return.
                "beq x18, x0, 5f",
                "xori x10, x10, -1",
                "xori x11, x11, -1",
                "addi x10, x10, 1",
                "sltiu x23, x10, 1",
                "add x11, x11, x23",
                "5:",
                "ret",
                "6:",
                // Underflow path.
                "mv x10, x0",
                "mv x11, x0",
                "ret",
                "7:",
                // Trap on NaN/Inf/overflow.
                "ebreak",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_trunc_f32_u(_x: f32) -> u64 {
        // Control-flow steps:
        // 1) Decode sign/exponent and reject negatives/NaN/Inf.
        // 2) Handle underflow to 0 and overflow traps.
        // 3) Build unsigned integer from significand/exponent.
        unsafe {
            core::arch::naked_asm!(
                // Step 1: load f32 bits and reject negative inputs for unsigned conversion.
                "fmv.x.w x20, f28",
                "srli x18, x20, 31",
                "beq x18, x0, 1f",
                "j 4f",
                "1:",
                // Step 2/3: exponent checks and significand shift assembly.
                "srli x21, x20, 23",
                "andi x21, x21, 255",
                "li x22, 255",
                "beq x21, x22, 5f",
                "addi x21, x21, -127",
                "blt x21, x0, 4f",
                "li x22, 63",
                "blt x22, x21, 5f",
                "lui x22, 0x800",
                "addi x22, x22, -1",
                "and x22, x20, x22",
                "lui x23, 0x800",
                "or x22, x22, x23",
                "addi x24, x21, -23",
                "blt x24, x0, 2f",
                "li x23, 32",
                "blt x24, x23, 3f",
                "addi x24, x24, -32",
                "sll x11, x22, x24",
                "mv x10, x0",
                "ret",
                "2:",
                "sub x24, x0, x24",
                "srl x10, x22, x24",
                "mv x11, x0",
                "ret",
                "3:",
                "sll x10, x22, x24",
                "li x23, 32",
                "sub x23, x23, x24",
                "srl x11, x22, x23",
                "ret",
                "4:",
                // Underflow/negative path returns 0 for this runtime behavior.
                "mv x10, x0",
                "mv x11, x0",
                "ret",
                "5:",
                // Trap on NaN/Inf/overflow.
                "ebreak",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_trunc_f64_s(_x: f64) -> i64 {
        // Control-flow steps:
        // 1) Materialize f64 bits and decode sign/exponent.
        // 2) Handle NaN/Inf, underflow-to-zero, and exact INT64_MIN boundary.
        // 3) Assemble 64-bit significand and shift per exponent.
        // 4) Apply sign and return, otherwise trap.
        unsafe {
            core::arch::naked_asm!(
                // Step 1: spill f64 to memory and read low/high words.
                "addi sp, sp, -8",
                "fsd f28, 0(sp)",
                "lw x20, 0(sp)",
                "lw x21, 4(sp)",
                "addi sp, sp, 8",
                "srli x18, x21, 31",
                "srli x22, x21, 20",
                "andi x22, x22, 2047",
                "li x23, 2047",
                "beq x22, x23, 8f",
                "addi x22, x22, -1023",
                "blt x22, x0, 7f",

                // Step 2: exp == 63 is only valid for -2^63 exactly.
                "li x23, 63",
                "bne x22, x23, 1f",
                "beq x18, x0, 8f",
                "lui x23, 0x100",
                "addi x23, x23, -1",
                "and x24, x21, x23",
                "or x24, x24, x20",
                "bne x24, x0, 8f",
                "mv x10, x0",
                "lui x11, 0x80000",
                "ret",

                "1:",
                // Step 3: assemble/significand shift path.
                "li x23, 63",
                "blt x23, x22, 8f",
                "lui x23, 0x100",
                "addi x23, x23, -1",
                "and x11, x21, x23",
                "lui x23, 0x100",
                "or x11, x11, x23",
                "mv x10, x20",
                "addi x24, x22, -52",
                "blt x24, x0, 2f",
                "beq x24, x0, 5f",
                "li x23, 32",
                "blt x24, x23, 3f",
                "addi x24, x24, -32",
                "sll x11, x10, x24",
                "mv x10, x0",
                "j 5f",
                "2:",
                "sub x24, x0, x24",
                "li x23, 32",
                "blt x24, x23, 4f",
                "addi x24, x24, -32",
                "srl x10, x11, x24",
                "mv x11, x0",
                "j 5f",
                "3:",
                "sll x25, x11, x24",
                "li x23, 32",
                "sub x23, x23, x24",
                "srl x26, x10, x23",
                "or x11, x25, x26",
                "sll x10, x10, x24",
                "j 5f",
                "4:",
                "li x23, 32",
                "sub x23, x23, x24",
                "sll x25, x11, x23",
                "srl x10, x10, x24",
                "li x23, 32",
                "sub x23, x23, x24",
                "srl x26, x11, x24",
                "or x10, x10, x25",
                "mv x11, x26",

                "5:",
                // Step 4: apply sign and return.
                "beq x18, x0, 6f",
                "xori x10, x10, -1",
                "xori x11, x11, -1",
                "addi x10, x10, 1",
                "sltiu x23, x10, 1",
                "add x11, x11, x23",
                "6:",
                "ret",
                "7:",
                // Underflow path.
                "mv x10, x0",
                "mv x11, x0",
                "ret",
                "8:",
                // Trap on NaN/Inf/overflow.
                "ebreak",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_trunc_f64_u(_x: f64) -> u64 {
        // Control-flow steps:
        // 1) Materialize f64 bits and reject negative inputs.
        // 2) Handle NaN/Inf, underflow-to-zero, and overflow checks.
        // 3) Assemble unsigned 64-bit value from significand/exponent shifts.
        unsafe {
            core::arch::naked_asm!(
                // Step 1: spill f64 to memory and read low/high words.
                "addi sp, sp, -8",
                "fsd f28, 0(sp)",
                "lw x20, 0(sp)",
                "lw x21, 4(sp)",
                "addi sp, sp, 8",
                "srli x18, x21, 31",
                "beq x18, x0, 1f",
                "j 6f",
                "1:",
                // Step 2/3: exponent checks and unsigned assembly path.
                "srli x22, x21, 20",
                "andi x22, x22, 2047",
                "li x23, 2047",
                "beq x22, x23, 7f",
                "addi x22, x22, -1023",
                "blt x22, x0, 6f",
                "li x23, 63",
                "blt x23, x22, 7f",
                "lui x23, 0x100",
                "addi x23, x23, -1",
                "and x11, x21, x23",
                "lui x23, 0x100",
                "or x11, x11, x23",
                "mv x10, x20",
                "addi x24, x22, -52",
                "blt x24, x0, 2f",
                "beq x24, x0, 4f",
                "li x23, 32",
                "blt x24, x23, 3f",
                "addi x24, x24, -32",
                "sll x11, x10, x24",
                "mv x10, x0",
                "ret",
                "2:",
                "sub x24, x0, x24",
                "li x23, 32",
                "blt x24, x23, 5f",
                "addi x24, x24, -32",
                "srl x10, x11, x24",
                "mv x11, x0",
                "ret",
                "3:",
                "sll x25, x11, x24",
                "li x23, 32",
                "sub x23, x23, x24",
                "srl x26, x10, x23",
                "or x11, x25, x26",
                "sll x10, x10, x24",
                "ret",
                "4:",
                "li x23, 32",
                "sub x23, x23, x24",
                "sll x25, x11, x23",
                "srl x10, x10, x24",
                "srl x11, x11, x24",
                "or x10, x10, x25",
                "ret",
                "5:",
                "li x23, 32",
                "sub x23, x23, x24",
                "sll x25, x11, x23",
                "srl x10, x10, x24",
                "or x10, x10, x25",
                "srl x11, x11, x24",
                "ret",
                "6:",
                // Underflow/negative path returns 0 for this runtime behavior.
                "mv x10, x0",
                "mv x11, x0",
                "ret",
                "7:",
                // Trap on NaN/Inf/overflow.
                "ebreak",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[inline(never)]
    #[unsafe(no_mangle)]
    extern "C" fn f32_convert_i64_s_rust_bits(lo: u32, hi: u32) -> u32 {
        let value = ((hi as i32 as i64) << 32) | ((lo as u64) as i64);
        (value as f32).to_bits()
    }

    #[cfg(target_arch = "riscv32")]
    #[inline(never)]
    #[unsafe(no_mangle)]
    extern "C" fn f32_convert_i64_u_rust_bits(lo: u32, hi: u32) -> u32 {
        let value = ((hi as u64) << 32) | (lo as u64);
        (value as f32).to_bits()
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f32_convert_i64_s(_x: i64) -> f32 {
        // Wrapper steps:
        // 1) Save caller-saved integer registers used by C ABI call sequence.
        // 2) Move custom helper ABI args (x27:x28) into a0:a1.
        // 3) Call Rust bit-conversion helper and move result bits to f10.
        // 4) Restore registers and return.
        unsafe {
            core::arch::naked_asm!(
                // Save ra and caller-saved scratch regs x10-x17 that are in the
                // allocatable range but not covered by the outer cm.push.
                "addi sp, sp, -36",
                "sw ra,  0(sp)",
                "sw x10, 4(sp)",
                "sw x11, 8(sp)",
                "sw x12, 12(sp)",
                "sw x13, 16(sp)",
                "sw x14, 20(sp)",
                "sw x15, 24(sp)",
                "sw x16, 28(sp)",
                "sw x17, 32(sp)",
                "mv a0, x27",
                "mv a1, x28",
                "call f32_convert_i64_s_rust_bits",
                "fmv.w.x f10, a0",
                "lw ra,  0(sp)",
                "lw x10, 4(sp)",
                "lw x11, 8(sp)",
                "lw x12, 12(sp)",
                "lw x13, 16(sp)",
                "lw x14, 20(sp)",
                "lw x15, 24(sp)",
                "lw x16, 28(sp)",
                "lw x17, 32(sp)",
                "addi sp, sp, 36",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f32_convert_i64_u(_x: u64) -> f32 {
        // Wrapper steps mirror f32_convert_i64_s, but for unsigned input interpretation.
        unsafe {
            core::arch::naked_asm!(
                "addi sp, sp, -36",
                "sw ra,  0(sp)",
                "sw x10, 4(sp)",
                "sw x11, 8(sp)",
                "sw x12, 12(sp)",
                "sw x13, 16(sp)",
                "sw x14, 20(sp)",
                "sw x15, 24(sp)",
                "sw x16, 28(sp)",
                "sw x17, 32(sp)",
                "mv a0, x27",
                "mv a1, x28",
                "call f32_convert_i64_u_rust_bits",
                "fmv.w.x f10, a0",
                "lw ra,  0(sp)",
                "lw x10, 4(sp)",
                "lw x11, 8(sp)",
                "lw x12, 12(sp)",
                "lw x13, 16(sp)",
                "lw x14, 20(sp)",
                "lw x15, 24(sp)",
                "lw x16, 28(sp)",
                "lw x17, 32(sp)",
                "addi sp, sp, 36",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f64_convert_i64_s(_x: i64) -> f64 {
        // Conversion steps:
        // 1) Convert low 32 bits as unsigned and high 32 bits as signed.
        // 2) Materialize 2^32 scale as f64.
        // 3) Combine hi*2^32 + lo in f64 domain.
        unsafe {
            core::arch::naked_asm!(
                "fcvt.d.wu f10, x27",
                "fcvt.d.w f11, x28",
                "lui x18, 0x80000",
                "fcvt.d.wu f12, x18",
                "fadd.d f12, f12, f12",
                "fmul.d f11, f11, f12",
                "fadd.d f10, f10, f11",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f64_convert_i64_u(_x: u64) -> f64 {
        // Conversion steps:
        // 1) Convert low/high 32-bit halves as unsigned.
        // 2) Materialize 2^32 scale as f64.
        // 3) Combine hi*2^32 + lo in f64 domain.
        unsafe {
            core::arch::naked_asm!(
                "fcvt.d.wu f10, x27",
                "fcvt.d.wu f11, x28",
                "lui x18, 0x80000",
                "fcvt.d.wu f12, x18",
                "fadd.d f12, f12, f12",
                "fmul.d f11, f11, f12",
                "fadd.d f10, f10, f11",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_div_s(_x: i64, _y: i64) -> i64 {
        // Custom helper ABI:
        // - lhs in x27:x28
        // - rhs in x29:x30
        // - output in x10:x11
        // Wasm semantics:
        // - trap on divide by zero
        // - trap on INT64_MIN / -1
        //
        // Register roles inside the algorithm:
        // - x22:x23 = |lhs| (unsigned numerator)
        // - x29:x30 = |rhs| (unsigned denominator)
        // - x24:x25 = running remainder
        // - x10:x11 = running quotient and final return value
        // - x18/x19 = original sign masks of lhs/rhs
        //
        // Control-flow steps:
        // 1) Early traps for rhs == 0 and signed overflow case.
        // 2) Capture signs and normalize both operands to absolute values.
        // 3) Run 64 iterations of unsigned long division.
        // 4) Re-apply quotient sign (lhs_sign XOR rhs_sign).
        // 5) Return quotient, or trap via ebreak.
        unsafe {
            core::arch::naked_asm!(
            // Step 1a: rhs == 0 -> trap.
                "or x18, x29, x30",
                "beq x18, x0, 8f",

            // Step 1b: overflow case lhs == INT64_MIN and rhs == -1 -> trap.
                "li x18, -1",
                "bne x29, x18, 1f",
                "bne x30, x18, 1f",
                "bne x27, x0, 1f",
                "lui x19, 0x80000",
                "bne x28, x19, 1f",
                "j 8f",

            // Step 2a: extract sign masks.
                "1:",
                "srai x18, x28, 31",
                "srai x19, x30, 31",

            // Step 2b: compute |lhs| into x22:x23 using two's complement when negative.
                "mv x22, x27",
                "mv x23, x28",
                "beq x18, x0, 2f",
                "xori x22, x22, -1",
                "xori x23, x23, -1",
                "addi x22, x22, 1",
                "sltiu x20, x22, 1",
                "add x23, x23, x20",

            // Step 2c: compute |rhs| in place into x29:x30.
                "2:",
                "beq x19, x0, 3f",
                "xori x29, x29, -1",
                "xori x30, x30, -1",
                "addi x29, x29, 1",
                "sltiu x20, x29, 1",
                "add x30, x30, x20",

            // Step 3a: initialize quotient and remainder for unsigned long division.
                "3:",
                "mv x24, x0",
                "mv x25, x0",
                "mv x10, x0",
                "mv x11, x0",
                "li x26, 64",
                "4:",
            // Step 3b: shift numerator left by one and capture outgoing top bit.
                "srli x20, x23, 31",
                "srli x21, x22, 31",
                "slli x23, x23, 1",
                "or x23, x23, x21",
                "slli x22, x22, 1",
            // Step 3c: shift remainder left and inject captured numerator bit.
                "srli x21, x24, 31",
                "slli x25, x25, 1",
                "or x25, x25, x21",
                "slli x24, x24, 1",
                "or x24, x24, x20",
            // Step 3d: shift quotient left by one.
                "srli x21, x10, 31",
                "slli x11, x11, 1",
                "or x11, x11, x21",
                "slli x10, x10, 1",
            // Step 3e: if remainder >= divisor, subtract divisor and set quotient low bit.
                "bltu x25, x30, 6f",
                "bltu x30, x25, 5f",
                "bltu x24, x29, 6f",
                "5:",
                "sub x20, x24, x29",
                "sltu x21, x24, x29",
                "sub x25, x25, x30",
                "sub x25, x25, x21",
                "mv x24, x20",
                "addi x10, x10, 1",
                "6:",
                // Step 3f: loop for exactly 64 bits.
                "addi x26, x26, -1",
                "bnez x26, 4b",

                // Step 4: apply signed-result polarity to the quotient.
                "xor x20, x18, x19",
                "beq x20, x0, 7f",
                "xori x10, x10, -1",
                "xori x11, x11, -1",
                "addi x10, x10, 1",
                "sltiu x21, x10, 1",
                "add x11, x11, x21",
                "7:",
                // Step 5a: normal return path.
                "ret",

                // Step 5b: trap path for wasm trap conditions.
                "8:",
                "ebreak",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_div_u(_x: u64, _y: u64) -> u64 {
        // Custom helper ABI:
        // - lhs in x27:x28
        // - rhs in x29:x30
        // - output in x10:x11
        // Wasm semantics: trap on divide by zero.
        //
        // Register roles inside the algorithm:
        // - x22:x23 = numerator
        // - x29:x30 = denominator
        // - x24:x25 = running remainder
        // - x10:x11 = running quotient and final return value
        //
        // Control-flow steps:
        // 1) Early trap for rhs == 0.
        // 2) Run 64-bit unsigned long division.
        // 3) Return quotient, or trap via ebreak.
        unsafe {
            core::arch::naked_asm!(
            // Step 1: rhs == 0 -> trap.
                "or x18, x29, x30",
                "beq x18, x0, 4f",

            // Step 2a: initialize numerator, denominator, quotient, remainder, and loop counter.
                "mv x22, x27",
                "mv x23, x28",
                "mv x24, x0",
                "mv x25, x0",
                "mv x10, x0",
                "mv x11, x0",
                "li x26, 64",
                "1:",
            // Step 2b: shift numerator left and capture outgoing bit.
                "srli x20, x23, 31",
                "srli x21, x22, 31",
                "slli x23, x23, 1",
                "or x23, x23, x21",
                "slli x22, x22, 1",
            // Step 2c: shift remainder left and inject captured bit.
                "srli x21, x24, 31",
                "slli x25, x25, 1",
                "or x25, x25, x21",
                "slli x24, x24, 1",
                "or x24, x24, x20",
            // Step 2d: shift quotient left.
                "srli x21, x10, 31",
                "slli x11, x11, 1",
                "or x11, x11, x21",
                "slli x10, x10, 1",
            // Step 2e: if remainder >= denominator, subtract and set quotient low bit.
                "bltu x25, x30, 3f",
                "bltu x30, x25, 2f",
                "bltu x24, x29, 3f",
                "2:",
                "sub x20, x24, x29",
                "sltu x21, x24, x29",
                "sub x25, x25, x30",
                "sub x25, x25, x21",
                "mv x24, x20",
                "addi x10, x10, 1",
                "3:",
                // Step 2f: loop for all 64 bits.
                "addi x26, x26, -1",
                "bnez x26, 1b",
                // Step 3a: normal return path.
                "ret",

                // Step 3b: trap path for divide-by-zero.
                "4:",
                "ebreak",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_rem_s(_x: i64, _y: i64) -> i64 {
        // Custom helper ABI:
        // - lhs in x27:x28
        // - rhs in x29:x30
        // - output in x10:x11
        // Wasm semantics:
        // - trap on divide by zero
        // - INT64_MIN % -1 = 0
        //
        // Register roles inside the algorithm:
        // - x22:x23 = |lhs| (unsigned numerator)
        // - x29:x30 = |rhs| (unsigned denominator)
        // - x24:x25 = running remainder and final magnitude
        // - x10:x11 = returned remainder (signed)
        // - x18/x19 = original sign masks of lhs/rhs
        //
        // Control-flow steps:
        // 1) Early trap for rhs == 0 and special-case return for INT64_MIN % -1.
        // 2) Capture signs and normalize operands to absolute values.
        // 3) Run unsigned long division remainder loop.
        // 4) Re-apply lhs sign to the remainder.
        // 5) Return remainder, or trap via ebreak.
        unsafe {
            core::arch::naked_asm!(
            // Step 1a: rhs == 0 -> trap.
                "or x18, x29, x30",
                "beq x18, x0, 8f",

            // Step 1b: special case INT64_MIN % -1 = 0.
                "li x18, -1",
                "bne x29, x18, 1f",
                "bne x30, x18, 1f",
                "bne x27, x0, 1f",
                "lui x19, 0x80000",
                "bne x28, x19, 1f",
                "mv x10, x0",
                "mv x11, x0",
                "ret",

            // Step 2a: extract sign masks.
                "1:",
                "srai x18, x28, 31",
                "srai x19, x30, 31",

            // Step 2b: compute |lhs| into x22:x23.
                "mv x22, x27",
                "mv x23, x28",
                "beq x18, x0, 2f",
                "xori x22, x22, -1",
                "xori x23, x23, -1",
                "addi x22, x22, 1",
                "sltiu x20, x22, 1",
                "add x23, x23, x20",

            // Step 2c: compute |rhs| in place into x29:x30.
                "2:",
                "beq x19, x0, 3f",
                "xori x29, x29, -1",
                "xori x30, x30, -1",
                "addi x29, x29, 1",
                "sltiu x20, x29, 1",
                "add x30, x30, x20",

            // Step 3a: initialize running remainder and loop counter.
                "3:",
                "mv x24, x0",
                "mv x25, x0",
                "li x26, 64",
                "4:",
            // Step 3b: shift numerator left and capture outgoing bit.
                "srli x20, x23, 31",
                "srli x21, x22, 31",
                "slli x23, x23, 1",
                "or x23, x23, x21",
                "slli x22, x22, 1",
            // Step 3c: shift remainder left and inject captured bit.
                "srli x21, x24, 31",
                "slli x25, x25, 1",
                "or x25, x25, x21",
                "slli x24, x24, 1",
                "or x24, x24, x20",
            // Step 3d: if remainder >= denominator, subtract denominator.
                "bltu x25, x30, 6f",
                "bltu x30, x25, 5f",
                "bltu x24, x29, 6f",
                "5:",
                "sub x20, x24, x29",
                "sltu x21, x24, x29",
                "sub x25, x25, x30",
                "sub x25, x25, x21",
                "mv x24, x20",
                "6:",
                // Step 3e: loop for all 64 bits.
                "addi x26, x26, -1",
                "bnez x26, 4b",

                // Step 4: result sign follows lhs sign.
                "mv x10, x24",
                "mv x11, x25",
                "beq x18, x0, 7f",
                "xori x10, x10, -1",
                "xori x11, x11, -1",
                "addi x10, x10, 1",
                "sltiu x21, x10, 1",
                "add x11, x11, x21",
                "7:",
                // Step 5a: normal return path.
                "ret",

                // Step 5b: trap path for divide-by-zero.
                "8:",
                "ebreak",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_rem_u(_x: u64, _y: u64) -> u64 {
        // Custom helper ABI:
        // - lhs in x27:x28
        // - rhs in x29:x30
        // - output in x10:x11
        // Wasm semantics: trap on divide by zero.
        //
        // Register roles inside the algorithm:
        // - x22:x23 = numerator
        // - x29:x30 = denominator
        // - x24:x25 = running remainder and final return value
        //
        // Control-flow steps:
        // 1) Early trap for rhs == 0.
        // 2) Run 64-bit unsigned long-division remainder loop.
        // 3) Return remainder, or trap via ebreak.
        unsafe {
            core::arch::naked_asm!(
            // Step 1: rhs == 0 -> trap.
                "or x18, x29, x30",
                "beq x18, x0, 4f",

            // Step 2a: initialize numerator, denominator, remainder, and loop counter.
                "mv x22, x27",
                "mv x23, x28",
                "mv x24, x0",
                "mv x25, x0",
                "li x26, 64",
                "1:",
            // Step 2b: shift numerator left and capture outgoing bit.
                "srli x20, x23, 31",
                "srli x21, x22, 31",
                "slli x23, x23, 1",
                "or x23, x23, x21",
                "slli x22, x22, 1",
            // Step 2c: shift remainder left and inject captured bit.
                "srli x21, x24, 31",
                "slli x25, x25, 1",
                "or x25, x25, x21",
                "slli x24, x24, 1",
                "or x24, x24, x20",
            // Step 2d: if remainder >= denominator, subtract denominator.
                "bltu x25, x30, 3f",
                "bltu x30, x25, 2f",
                "bltu x24, x29, 3f",
                "2:",
                "sub x20, x24, x29",
                "sltu x21, x24, x29",
                "sub x25, x25, x30",
                "sub x25, x25, x21",
                "mv x24, x20",
                "3:",
                // Step 2e: loop for all 64 bits.
                "addi x26, x26, -1",
                "bnez x26, 1b",
                // Step 3a: normal return path.
                "mv x10, x24",
                "mv x11, x25",
                "ret",

                // Step 3b: trap path for divide-by-zero.
                "4:",
                "ebreak",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_shl(_x: i64, _y: i64) -> i64 {
        // Custom helper ABI:
        // - lhs in x27:x28
        // - rhs in x29:x30
        // - output in x10:x11
        // Behavior: use rhs & 63 as the shift amount.
        //
        // Control-flow steps:
        // 1) Mask shift amount to 0..63.
        // 2) Handle shift-by-0 fast path.
        // 3) Handle >=32 and <32 shift ranges separately.
        unsafe {
            core::arch::naked_asm!(
                "andi x18, x29, 63",
                "beq x18, x0, 2f",
                "li x19, 32",
                "bltu x18, x19, 1f",
                "addi x18, x18, -32",
                "sll x11, x27, x18",
                "mv x10, x0",
                "ret",
                "1:",
                "sub x19, x19, x18",
                "sll x11, x28, x18",
                "srl x20, x27, x19",
                "or x11, x11, x20",
                "sll x10, x27, x18",
                "ret",
                "2:",
                "mv x10, x27",
                "mv x11, x28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_shr_s(_x: i64, _y: i64) -> i64 {
        // Custom helper ABI:
        // - lhs in x27:x28
        // - rhs in x29:x30
        // - output in x10:x11
        // Behavior: use rhs & 63 as the shift amount.
        //
        // Control-flow steps mirror i64_shl, but with arithmetic right-shift semantics.
        unsafe {
            core::arch::naked_asm!(
                // Step 1: normalize shift amount to 0..63.
                "andi x18, x29, 63",
                // Step 2: shift-by-0 fast path.
                "beq x18, x0, 2f",
                "li x19, 32",
                // Step 3: split by <32 vs >=32 and compose result words.
                "bltu x18, x19, 1f",
                "addi x18, x18, -32",
                "sra x10, x28, x18",
                "srai x11, x28, 31",
                "ret",
                "1:",
                "sub x19, x19, x18",
                "srl x10, x27, x18",
                "sll x20, x28, x19",
                "or x10, x10, x20",
                "sra x11, x28, x18",
                "ret",
                "2:",
                "mv x10, x27",
                "mv x11, x28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_shr_u(_x: u64, _y: u64) -> u64 {
        // Custom helper ABI:
        // - lhs in x27:x28
        // - rhs in x29:x30
        // - output in x10:x11
        // Behavior: use rhs & 63 as the shift amount.
        //
        // Control-flow steps mirror i64_shl, but with logical right-shift semantics.
        unsafe {
            core::arch::naked_asm!(
                // Step 1: normalize shift amount to 0..63.
                "andi x18, x29, 63",
                // Step 2: shift-by-0 fast path.
                "beq x18, x0, 2f",
                "li x19, 32",
                // Step 3: split by <32 vs >=32 and compose result words.
                "bltu x18, x19, 1f",
                "addi x18, x18, -32",
                "srl x10, x28, x18",
                "mv x11, x0",
                "ret",
                "1:",
                "sub x19, x19, x18",
                "srl x10, x27, x18",
                "sll x20, x28, x19",
                "or x10, x10, x20",
                "srl x11, x28, x18",
                "ret",
                "2:",
                "mv x10, x27",
                "mv x11, x28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_rotl(_x: u64, _y: u64) -> u64 {
        // Custom helper ABI:
        // - lhs in x27:x28
        // - rhs in x29:x30
        // - output in x10:x11
        // Behavior: use rhs & 63 as the rotate amount.
        //
        // Control-flow steps:
        // 1) Mask rotate amount to 0..63.
        // 2) Handle rotate-by-0 fast path.
        // 3) Split into <32, ==32, and >32 cases and stitch words accordingly.
        unsafe {
            core::arch::naked_asm!(
                // Step 1: normalize rotate amount to 0..63.
                "andi x18, x29, 63",
                // Step 2: rotate-by-0 fast path.
                "beq x18, x0, 3f",
                "li x19, 32",
                // Step 3: split into <32, ==32, and >32 composition paths.
                "bltu x18, x19, 1f",
                "addi x18, x18, -32",
                "beq x18, x0, 2f",
                "sub x19, x19, x18",
                "sll x10, x28, x18",
                "srl x20, x27, x19",
                "or x10, x10, x20",
                "sll x11, x27, x18",
                "srl x20, x28, x19",
                "or x11, x11, x20",
                "ret",
                "1:",
                "sub x19, x19, x18",
                "sll x10, x27, x18",
                "srl x20, x28, x19",
                "or x10, x10, x20",
                "sll x11, x28, x18",
                "srl x20, x27, x19",
                "or x11, x11, x20",
                "ret",
                "2:",
                "mv x10, x28",
                "mv x11, x27",
                "ret",
                "3:",
                "mv x10, x27",
                "mv x11, x28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn i64_rotr(_x: u64, _y: u64) -> u64 {
        // Custom helper ABI:
        // - lhs in x27:x28
        // - rhs in x29:x30
        // - output in x10:x11
        // Behavior: use rhs & 63 as the rotate amount.
        //
        // Control-flow steps mirror i64_rotl with opposite bit-flow direction.
        unsafe {
            core::arch::naked_asm!(
                // Step 1: normalize rotate amount to 0..63.
                "andi x18, x29, 63",
                // Step 2: rotate-by-0 fast path.
                "beq x18, x0, 3f",
                "li x19, 32",
                // Step 3: split into <32, ==32, and >32 composition paths.
                "bltu x18, x19, 1f",
                "addi x18, x18, -32",
                "beq x18, x0, 2f",
                "sub x19, x19, x18",
                "srl x10, x28, x18",
                "sll x20, x27, x19",
                "or x10, x10, x20",
                "srl x11, x27, x18",
                "sll x20, x28, x19",
                "or x11, x11, x20",
                "ret",
                "1:",
                "sub x19, x19, x18",
                "srl x10, x27, x18",
                "sll x20, x28, x19",
                "or x10, x10, x20",
                "srl x11, x28, x18",
                "sll x20, x27, x19",
                "or x11, x11, x20",
                "ret",
                "2:",
                "mv x10, x28",
                "mv x11, x27",
                "ret",
                "3:",
                "mv x10, x27",
                "mv x11, x28",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f32_max(_x: f32, _y: f32) -> f32 {
        // Custom helper ABI:
        // - inputs in f28/f29
        // - output in f10
        // Behavior: if either input is NaN, return canonical NaN; else max with +0 tie behavior.
        //
        // Steps:
        // 1) Detect NaN using x != x test (feq with self).
        // 2) Return canonical NaN if any operand is NaN.
        // 3) Otherwise perform fmax.s.
        unsafe {
            core::arch::naked_asm!(
                "feq.s x18, f28, f28", // x is not equal to itself if it is NaN
                "beq x18, x0, 2f",
                "feq.s x18, f29, f29", // y is not equal to itself if it is NaN
                "beq x18, x0, 2f",
                "fmax.s f10, f28, f29",
                "ret",
                "2:",
                "lui x18, 0x7fc00",
                "fmv.w.x f10, x18",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f32_min(_x: f32, _y: f32) -> f32 {
        // Custom helper ABI:
        // - inputs in f28/f29
        // - output in f10
        // Behavior: if either input is NaN, return canonical NaN; else min with -0 tie behavior.
        //
        // Steps mirror f32_max, using fmin.s on non-NaN inputs.
        unsafe {
            core::arch::naked_asm!(
                "feq.s x18, f28, f28",
                "beq x18, x0, 2f",
                "feq.s x18, f29, f29",
                "beq x18, x0, 2f",
                "fmin.s f10, f28, f29",
                "ret",
                "2:",
                "lui x18, 0x7fc00",
                "fmv.w.x f10, x18",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f64_max(_x: f64, _y: f64) -> f64 {
        // Custom helper ABI:
        // - inputs in f28/f29
        // - output in f10
        // Behavior: if either input is NaN, return canonical NaN; else max with +0 tie behavior.
        //
        // Steps:
        // 1) Detect NaN using x != x test.
        // 2) Return canonical NaN if needed.
        // 3) Otherwise perform fmax.d.
        unsafe {
            core::arch::naked_asm!(
                "feq.d x18, f28, f28", // x is not equal to itself if it is NaN
                "beq x18, x0, 2f",
                "feq.d x18, f29, f29", // y is not equal to itself if it is NaN
                "beq x18, x0, 2f",
                "fmax.d f10, f28, f29",
                "ret",
                "2:",
                // Build +0.0 and do 0.0/0.0 to obtain canonical NaN in f10.
                "fcvt.d.w f10, x0",
                "fdiv.d f10, f10, f10",
                "ret",
            );
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[naked]
    pub extern "C" fn f64_min(_x: f64, _y: f64) -> f64 {
        // Custom helper ABI:
        // - inputs in f28/f29
        // - output in f10
        // Behavior: if either input is NaN, return canonical NaN; else min with -0 tie behavior.
        //
        // Steps mirror f64_max, using fmin.d on non-NaN inputs.
        unsafe {
            core::arch::naked_asm!(
                "feq.d x18, f28, f28",
                "beq x18, x0, 2f",
                "feq.d x18, f29, f29",
                "beq x18, x0, 2f",
                "fmin.d f10, f28, f29",
                "ret",
                "2:",
                // Build +0.0 and do 0.0/0.0 to obtain canonical NaN in f10.
                "fcvt.d.w f10, x0",
                "fdiv.d f10, f10, f10",
                "ret",
            );
        }
    }


    /// This subroutine is to be called prior to an indirect call in a wasm function. It performs the necessary checks before running the indirect call to ensure
    /// type safety. It checks that the provided table offset does not exceed the table size (in case sandboxing is enabled) and checks whether the referenced function
    /// matches the statically annotated type.
    pub extern "C" fn compare_subtypes(types: *const SubType, table_type_indices: *const u32, table_offset :u32, target_type_index : u32, _table_size: u32)  {
        #[cfg(feature="address-masking")]
        assert!(table_offset < _table_size);
        let type_index =  unsafe{ *table_type_indices.wrapping_add(table_offset as usize)};
        let ty = types.wrapping_add(type_index as usize);
        let target_ty = types.wrapping_add(target_type_index as usize);
        unsafe {
            assert_eq!((*ty).composite_type, (*target_ty).composite_type)
        }
    }

    /// This subroutine is to be called for implementing the memory.grow instruction.
    /// It checks whether the new expected size of the memory (in pages) exceeds the maximum size allowed.
    /// The maximum allowed size is dependent both of the allocated space for the linear memory and the maximum indicated by the wasm module.
    pub extern "C" fn grow_memory(current_size: &mut u32, grow_size:u32, maximum_size:u32) -> u32 {
        let previous_size = *current_size;
        match previous_size.checked_add(grow_size){
            Some (new_size) if new_size <= maximum_size => {
                *current_size = new_size;
                previous_size
            }
            _ => u32::MAX
        }
    }

    // wrap_env!{
    //     fn __write__(address: u32, value: u32){
    //         unsafe {
    //             let mem_ptr = address as *mut u32;
    //             *mem_ptr = value;
    //         }
    //     }
    // }

    // wrap_env!{
    //     fn __read__(address:u32) -> u32 {
    //         unsafe {
    //             let mem_ptr = address as *const u32;
    //             *mem_ptr
    //         }
    //     }
    // }

}
