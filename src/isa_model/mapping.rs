use alloc::vec::Vec;
use alloc::vec;

use crate::isa_model::{
    Const12, Const20, FPRegister, FRAME_POINTER, GLOBAL_BASE, GPRegister, Immediate,
    MapperLocation, RegisterPair, RiscvInstr, STACK_POINTER, ValueSize, ValueType,
};
use crate::translator::{StackHeight, Translator};

fn fits_signed_12(value: i32) -> bool {
    (-2048..=2047).contains(&value)
}

fn encode_offset(offset: i32) -> Const12 {
    assert!(fits_signed_12(offset), "offset {offset} does not fit in a 12-bit signed immediate");
    Const12::new(offset as i16)
}

fn stack_offset(translator: &mut Translator<'_, '_>, logical_offset: i32) -> i32 {
    translator.stack_logical_offset_to_imm(StackHeight::from(logical_offset as usize)) as i32
}

fn split_immediate(value: u32) -> (i32, i16) {
    let signed_value = value as i32;
    let upper = (signed_value.wrapping_add(0x800)) >> 12;
    let lower = (signed_value & 0xFFF) as i16;
    (upper, lower)
}

fn load_word(translator: &mut Translator<'_, '_>, dest: GPRegister, base: GPRegister, offset: i32) {
    translator.push_instruction(RiscvInstr::LW {
        rd: dest,
        rs1: base,
        imm: encode_offset(offset),
    });
}

// TODO: maybe some checks with the bitmask are needed
fn store_word(translator: &mut Translator<'_, '_>, src: GPRegister, base: GPRegister, offset: i32) {
    translator.push_instruction(RiscvInstr::SW {
        rs2: src,
        rs1: base,
        imm: encode_offset(offset),
    });
}

fn load_fp(
    translator: &mut Translator<'_, '_>,
    dest: FPRegister,
    base: GPRegister,
    offset: i32,
    val_size: ValueSize,
) {
    translator.push_instruction(match val_size {
        ValueSize::Word => RiscvInstr::FLW {
            rd: dest,
            rs1: base,
            imm: encode_offset(offset),
        },
        ValueSize::DoubleWord => RiscvInstr::FLD {
            rd: dest,
            rs1: base,
            imm: encode_offset(offset),
        },
    });
}

fn store_fp(
    translator: &mut Translator<'_, '_>,
    src: FPRegister,
    base: GPRegister,
    offset: i32,
    val_size: ValueSize,
) {
    translator.push_instruction(match val_size {
        ValueSize::Word => RiscvInstr::FSW {
            rs2: src,
            rs1: base,
            imm: encode_offset(offset),
        },
        ValueSize::DoubleWord => RiscvInstr::FSD {
            rs2: src,
            rs1: base,
            imm: encode_offset(offset),
        },
    });
}

fn materialize_u32(translator: &mut Translator<'_, '_>, dest: GPRegister, value: u32) {
    let signed_value = value as i32;
    if fits_signed_12(signed_value) {
        translator.push_instruction(RiscvInstr::ADDI {
            rd: dest,
            rs1: GPRegister(0),
            imm: encode_offset(signed_value),
        });
        return;
    }

    let (upper, lower) = split_immediate(value);
    translator.push_instruction(RiscvInstr::LUI {
        rd: dest,
        imm: Const20::new(upper),
    });

    if lower != 0 {
        translator.push_instruction(RiscvInstr::ADDI {
            rd: dest,
            rs1: dest,
            imm: Const12::new(lower),
        });
    }
}

// We need to materialize the bit pattern into a GP register first because 
// RISC-V does not have an instruction to load an immediate directly into an FP register
fn materialize_f32_bits(
    translator: &mut Translator<'_, '_>,
    dest: FPRegister,
    value: u32,
    scratch_variable_map: &mut Vec<MapperLocation>,
    used_registers: &Vec<MapperLocation>,
) {
    let gp_temp = translator.next_available_gp_register(scratch_variable_map, used_registers);
    materialize_u32(translator, gp_temp, value);
    translator.push_instruction(RiscvInstr::FMVWX { rd: dest, rs1: gp_temp });
}

fn materialize_f64_bits(
    translator: &mut Translator<'_, '_>,
    dest: FPRegister,
    value: u64,
    scratch_variable_map: &mut Vec<MapperLocation>,
    used_registers: &Vec<MapperLocation>,
) {
    let pair = translator.next_available_register_pair(scratch_variable_map, used_registers);
    materialize_u32(translator, pair.low(), value as u32);
    materialize_u32(translator, pair.high(), (value >> 32) as u32);
    
    // Place scratch below all live values and round its logical height to an
    // 8-byte boundary so the resulting address is valid for FLD/FSD.
    let scratch_height = StackHeight::from((translator.modeled_stack_height.to_usize() + 15) & !7);
    let scratch_offset = translator.stack_logical_offset_to_imm(scratch_height) as i32;
    store_word(translator, pair.low(), STACK_POINTER, scratch_offset);
    store_word(translator, pair.high(), STACK_POINTER, scratch_offset + 4);
    load_fp(translator, dest, STACK_POINTER, scratch_offset, ValueSize::DoubleWord);
}



fn store_to_frame_like(
    source: &MapperLocation,
    target: &MapperLocation,
    translator: &mut Translator<'_, '_>,
    scratch_variable_map: &mut Vec<MapperLocation>,
    used_registers: &Vec<MapperLocation>,
    base_register: GPRegister,
    base_offset: i32,
    val_type: ValueType,
) -> MapperLocation {
    match val_type {
        ValueType::I32 => {
            let register = source.map_to_gp_register(None, translator, scratch_variable_map, used_registers);
            store_word(translator, register, base_register, base_offset);
            target.clone()
        }
        ValueType::I64 => {
            let pair = source.map_to_register_pair(None, translator, scratch_variable_map, used_registers);
            store_word(translator, pair.low(), base_register, base_offset);
            store_word(translator, pair.high(), base_register, base_offset + 4);
            target.clone()
        }
        ValueType::F32 => match source {
            MapperLocation::Immediate(Immediate::F32(bits)) => {
                let fp_temp = translator.next_available_fp_register(scratch_variable_map, used_registers);
                materialize_f32_bits(translator, fp_temp, *bits, scratch_variable_map, used_registers);
                store_fp(translator, fp_temp, base_register, base_offset, ValueSize::Word);
                target.clone()
            }
            _ => {
                let register = source.map_to_fp_register(None, ValueSize::Word, translator, scratch_variable_map, used_registers);
                store_fp(translator, register, base_register, base_offset, ValueSize::Word);
                target.clone()
            }
        },
        ValueType::F64 => match source {
            MapperLocation::Immediate(Immediate::F64(bits)) => {
                let fp_temp = translator.next_available_fp_register(scratch_variable_map, used_registers);
                materialize_f64_bits(translator, fp_temp, *bits, scratch_variable_map, used_registers);
                store_fp(translator, fp_temp, base_register, base_offset, ValueSize::DoubleWord);
                target.clone()
            }
            _ => {
                let register = source.map_to_fp_register(None, ValueSize::DoubleWord, translator, scratch_variable_map, used_registers);
                store_fp(translator, register, base_register, base_offset, ValueSize::DoubleWord);
                target.clone()
            }
        },
    }
}

impl MapperLocation {
    pub(crate) fn map_to_location(
        &self,
        target: &MapperLocation,
        translator: &mut Translator<'_, '_>,
        scratch_variable_map: &mut Vec<MapperLocation>,
        used_registers: &Vec<MapperLocation>,
    ) -> MapperLocation {
        if self == target {
            return self.clone();
        }

        match target {
            MapperLocation::GPRegister(register) => {
                MapperLocation::GPRegister(self.map_to_gp_register(Some(*register), translator, scratch_variable_map, used_registers))
            }
            MapperLocation::RegisterPair(register_pair) => {
                MapperLocation::RegisterPair(self.map_to_register_pair(Some(*register_pair), translator, scratch_variable_map, used_registers))
            }
            MapperLocation::Frame { offset, val_type } => store_to_frame_like(
                self,
                target,
                translator,
                scratch_variable_map,
                used_registers,
                FRAME_POINTER,
                i32::from(*offset),
                *val_type,
            ),
            MapperLocation::Stack { offset, val_type } => {
                let offset = stack_offset(translator, *offset);
                store_to_frame_like(
                    self,
                    target,
                    translator,
                    scratch_variable_map,
                    used_registers,
                    STACK_POINTER,
                    offset,
                    *val_type,
                )
            }
            MapperLocation::Global { offset, val_type } => store_to_frame_like(
                self,
                target,
                translator,
                scratch_variable_map,
                used_registers,
                GLOBAL_BASE,
                i32::try_from(*offset).expect("global offset out of range for baseline mapper"),
                *val_type,
            ),
            MapperLocation::FPRegister { .. } => {
                match self.get_type() {
                    ValueType::F32 => {
                        if let MapperLocation::FPRegister { reg, .. } = target {
                            MapperLocation::FPRegister {
                                reg: self.map_to_fp_register(Some(*reg), ValueSize::Word, translator, scratch_variable_map, used_registers),
                                val_size: ValueSize::Word,
                            }
                        } else {
                            unreachable!()
                        }
                    }
                    ValueType::F64 => {
                        if let MapperLocation::FPRegister { reg, .. } = target {
                            MapperLocation::FPRegister {
                                reg: self.map_to_fp_register(Some(*reg), ValueSize::DoubleWord, translator, scratch_variable_map, used_registers),
                                val_size: ValueSize::DoubleWord,
                            }
                        } else {
                            unreachable!()
                        }
                    }
                    value_type => panic!("cannot map {value_type:?} into an FP register"),
                }
            }
            MapperLocation::Immediate(_) => panic!("cannot target an immediate location"),
        }
    }

    pub(crate) fn map_to_gp_register(
        &self,
        target: Option<GPRegister>,
        translator: &mut Translator<'_, '_>,
        scratch_variable_map: &mut Vec<MapperLocation>,
        used_registers: &Vec<MapperLocation>,
    ) -> GPRegister {
        let destination = match (self, target) {
            (MapperLocation::GPRegister(register), None) => return *register,
            (_, Some(destination)) => destination,
            (_, None) => translator.next_available_gp_register(scratch_variable_map, used_registers),
        };

        match self {
            MapperLocation::GPRegister(register) => {
                register.move_to(destination, translator);
                destination
            }
            MapperLocation::Immediate(Immediate::I32(value)) => {
                materialize_u32(translator, destination, *value);
                destination
            }
            MapperLocation::Frame { offset, val_type } => match val_type {
                ValueType::I32 => {
                    load_word(translator, destination, FRAME_POINTER, i32::from(*offset));
                    destination
                }
                _ => panic!("cannot map {val_type:?} directly to a GP register in the baseline mapper"),
            },
            MapperLocation::Stack { offset, val_type } => match val_type {
                ValueType::I32 => {
                    let offset = stack_offset(translator, *offset);
                    load_word(translator, destination, STACK_POINTER, offset);
                    destination
                }
                _ => panic!("cannot map {val_type:?} directly to a GP register in the baseline mapper"),
            },
            MapperLocation::Global { offset, val_type } => match val_type {
                ValueType::I32 => {
                    load_word(
                        translator,
                        destination,
                        GLOBAL_BASE,
                        i32::try_from(*offset).expect("global offset out of range for baseline mapper"),
                    );
                    destination
                }
                _ => panic!("cannot map {val_type:?} directly to a GP register in the baseline mapper"),
            },
            MapperLocation::RegisterPair(register_pair) => {
                register_pair.low().move_to(destination, translator);
                destination
            }
            MapperLocation::Immediate(Immediate::I64(_))
            | MapperLocation::Immediate(Immediate::F32(_))
            | MapperLocation::Immediate(Immediate::F64(_))
            | MapperLocation::FPRegister { .. } => {
                panic!("unsupported narrowing into GP register in baseline mapper")
            }
        }
    }

    pub(crate) fn map_to_fp_register(
        &self,
        target: Option<FPRegister>,
        val_size: ValueSize,
        translator: &mut Translator<'_, '_>,
        scratch_variable_map: &mut Vec<MapperLocation>,
        used_registers: &Vec<MapperLocation>,
    ) -> FPRegister {
        let destination = match (self, target) {
            (MapperLocation::FPRegister { reg, .. }, None) => return *reg,
            (_, Some(destination)) => destination,
            (_, None) => translator.next_available_fp_register(scratch_variable_map, used_registers),
        };

        match self {
            MapperLocation::FPRegister { reg, val_size: current_size } => {
                if *current_size != val_size {
                    panic!("cannot map {current_size:?} value into {val_size:?} FP register target")
                }
                reg.move_to(destination, val_size, translator);
                destination
            }
            MapperLocation::Immediate(Immediate::F32(bits)) => {
                if val_size != ValueSize::Word {
                    panic!("cannot materialize an f32 immediate into a {val_size:?} FP register")
                }
                materialize_f32_bits(translator, destination, *bits, scratch_variable_map, used_registers);
                destination
            }
            MapperLocation::Immediate(Immediate::F64(bits)) => {
                if val_size != ValueSize::DoubleWord {
                    panic!("cannot materialize an f64 immediate into a {val_size:?} FP register")
                }
                materialize_f64_bits(translator, destination, *bits, scratch_variable_map, used_registers);
                destination
            }
            MapperLocation::Frame { offset, val_type } => match val_type {
                ValueType::F32 => {
                    load_fp(translator, destination, FRAME_POINTER, i32::from(*offset), ValueSize::Word);
                    destination
                }
                ValueType::F64 => {
                    load_fp(translator, destination, FRAME_POINTER, i32::from(*offset), ValueSize::DoubleWord);
                    destination
                }
                _ => panic!("cannot map {val_type:?} directly to an FP register in the baseline mapper"),
            },
            MapperLocation::Stack { offset, val_type } => match val_type {
                ValueType::F32 => {
                    let offset = stack_offset(translator, *offset);
                    load_fp(translator, destination, STACK_POINTER, offset, ValueSize::Word);
                    destination
                }
                ValueType::F64 => {
                    let offset = stack_offset(translator, *offset);
                    load_fp(translator, destination, STACK_POINTER, offset, ValueSize::DoubleWord);
                    destination
                }
                _ => panic!("cannot map {val_type:?} directly to an FP register in the baseline mapper"),
            },
            MapperLocation::Global { offset, val_type } => match val_type {
                ValueType::F32 => {
                    load_fp(
                        translator,
                        destination,
                        GLOBAL_BASE,
                        i32::try_from(*offset).expect("global offset out of range for baseline mapper"),
                        ValueSize::Word,
                    );
                    destination
                }
                ValueType::F64 => {
                    load_fp(
                        translator,
                        destination,
                        GLOBAL_BASE,
                        i32::try_from(*offset).expect("global offset out of range for baseline mapper"),
                        ValueSize::DoubleWord,
                    );
                    destination
                }
                _ => panic!("cannot map {val_type:?} directly to an FP register in the baseline mapper"),
            },
            MapperLocation::Immediate(Immediate::I32(_))
            | MapperLocation::Immediate(Immediate::I64(_))
            | MapperLocation::GPRegister(_)
            | MapperLocation::RegisterPair(_) => {
                panic!("integer to FP register mapping is not implemented in the baseline mapper")
            }
        }
    }

    pub(crate) fn map_to_register_pair(
        &self,
        target: Option<RegisterPair>,
        translator: &mut Translator<'_, '_>,
        scratch_variable_map: &mut Vec<MapperLocation>,
        used_registers: &Vec<MapperLocation>,
    ) -> RegisterPair {
        let destination = target.unwrap_or_else(|| translator.next_available_register_pair(scratch_variable_map, used_registers));

        match self {
            MapperLocation::RegisterPair(register_pair) => {
                // If source.high overlaps dest.low, move high first to avoid clobbering.
                if destination.low() == register_pair.high() {
                    register_pair.high().move_to(destination.high(), translator);
                    register_pair.low().move_to(destination.low(), translator);
                } else {
                    register_pair.low().move_to(destination.low(), translator);
                    register_pair.high().move_to(destination.high(), translator);
                }
                destination
            }
            MapperLocation::Immediate(Immediate::I64(value)) => {
                materialize_u32(translator, destination.low(), *value as u32);
                materialize_u32(translator, destination.high(), (*value >> 32) as u32);
                destination
            }
            MapperLocation::Frame { offset, val_type } => match val_type {
                ValueType::I64 => {
                    load_word(translator, destination.low(), FRAME_POINTER, i32::from(*offset));
                    load_word(translator, destination.high(), FRAME_POINTER, i32::from(*offset) + 4);
                    destination
                }
                _ => panic!("cannot map {val_type:?} directly to a register pair in the baseline mapper"),
            },
            MapperLocation::Stack { offset, val_type } => match val_type {
                ValueType::I64 => {
                    let offset = stack_offset(translator, *offset);
                    load_word(translator, destination.low(), STACK_POINTER, offset);
                    load_word(translator, destination.high(), STACK_POINTER, offset + 4);
                    destination
                }
                _ => panic!("cannot map {val_type:?} directly to a register pair in the baseline mapper"),
            },
            MapperLocation::Global { offset, val_type } => match val_type {
                ValueType::I64 => {
                    let offset = i32::try_from(*offset).expect("global offset out of range for baseline mapper");
                    load_word(translator, destination.low(), GLOBAL_BASE, offset);
                    load_word(translator, destination.high(), GLOBAL_BASE, offset + 4);
                    destination
                }
                _ => panic!("cannot map {val_type:?} directly to a register pair in the baseline mapper"),
            },
            MapperLocation::GPRegister(register) => {
                register.move_to(destination.low(), translator);
                materialize_u32(translator, destination.high(), 0);
                destination
            }
            MapperLocation::Immediate(Immediate::I32(value)) => {
                materialize_u32(translator, destination.low(), *value);
                materialize_u32(translator, destination.high(), 0);
                destination
            }
            MapperLocation::Immediate(Immediate::F32(_))
            | MapperLocation::Immediate(Immediate::F64(_))
            | MapperLocation::FPRegister { .. } => {
                panic!("floating-point register-pair mapping is not implemented in the baseline mapper")
            }
        }
    }

    // lower 32 bits, not always the half
    pub(crate) fn lower_half (&self) -> Self {
        match self {
            MapperLocation::GPRegister(register) => MapperLocation::GPRegister(*register),
            MapperLocation::RegisterPair(RegisterPair{low, ..}) => MapperLocation::GPRegister(*low),
            MapperLocation::FPRegister { reg, ..} => MapperLocation::FPRegister { reg: *reg, val_size: ValueSize::Word },
            MapperLocation::Frame { offset, val_type} => MapperLocation::Frame {offset:*offset, val_type: *&val_type.half_size()},
            MapperLocation::Immediate(immediate) => MapperLocation::Immediate(immediate.lower_half()),
            MapperLocation::Stack { offset, val_type } => MapperLocation::Stack { offset: *offset, val_type: val_type.half_size() },
            MapperLocation::Global { offset, val_type } => MapperLocation::Global { offset: *offset, val_type: val_type.half_size() },
        }
    }
}

pub(crate) trait LocationCouple {
    fn map_to_gp_registers(self, translator: &mut Translator<'_, '_>, scratch_variable_map : &mut Vec<MapperLocation>) -> (GPRegister, GPRegister);
    fn map_to_fp_registers(self, translator: &mut Translator<'_, '_>, scratch_variable_map : &mut Vec<MapperLocation>) -> (FPRegister, FPRegister);
    fn map_to_register_pair(self, translator: &mut Translator<'_, '_>, scratch_variable_map : &mut Vec<MapperLocation>) -> (RegisterPair, RegisterPair);
}

impl LocationCouple for (&MapperLocation, &MapperLocation) {
    fn map_to_gp_registers(self, translator: &mut Translator<'_, '_>, scratch_variable_map : &mut Vec<MapperLocation>) -> (GPRegister, GPRegister) {
        let (lhs,rhs) = self;
        let rhs_register = rhs.map_to_gp_register(None, translator, scratch_variable_map, &vec![lhs.clone()]);
        let lhs_register = lhs.map_to_gp_register(None, translator, scratch_variable_map, &vec![MapperLocation::GPRegister(rhs_register)]);
        (lhs_register, rhs_register)    
    }

    fn map_to_fp_registers(self, translator: &mut Translator<'_, '_>, scratch_variable_map : &mut Vec<MapperLocation>) -> (FPRegister, FPRegister) {
        let (lhs,rhs) = self;
        let val_size = lhs.get_type().as_valuesize();
        debug_assert_eq!(val_size, rhs.get_type().as_valuesize(), "type mismatch between lhs and rhs when mapping to FP registers");
        let rhs_register = rhs.map_to_fp_register(None, val_size, translator, scratch_variable_map, &vec![lhs.clone()]);
        let lhs_register = lhs.map_to_fp_register(None, val_size, translator, scratch_variable_map, &vec![MapperLocation::FPRegister { reg: rhs_register, val_size }]);
        (lhs_register, rhs_register)
    }

    fn map_to_register_pair(self, translator: &mut Translator<'_, '_>, scratch_variable_map : &mut Vec<MapperLocation>) -> (RegisterPair, RegisterPair) {
        let (lhs,rhs) = self;
        let rhs_register = rhs.map_to_register_pair(None, translator, scratch_variable_map, &vec![lhs.clone()]);
        let lhs_register = lhs.map_to_register_pair(None, translator, scratch_variable_map, &vec![MapperLocation::RegisterPair(rhs_register)]);
        (lhs_register, rhs_register)
    }
}

impl GPRegister {
    pub(crate) fn map_to_location(
        &self,
        potential_target: Option<&MapperLocation>,
        translator: &mut Translator<'_, '_>,
        scratch_variable_map: &mut Vec<MapperLocation>,
    ) -> MapperLocation {
        match potential_target {
            Some(target) => MapperLocation::GPRegister(*self).map_to_location(target, translator, scratch_variable_map, &vec![]),
            None => MapperLocation::GPRegister(*self),
        }
    }
}

impl RegisterPair {
    pub(crate) fn map_to_location(
        &self,
        potential_target: Option<&MapperLocation>,
        translator: &mut Translator<'_, '_>,
        scratch_variable_map: &mut Vec<MapperLocation>,
    ) -> MapperLocation {
        match potential_target {
            Some(target) => MapperLocation::RegisterPair(*self).map_to_location(target, translator, scratch_variable_map, &vec![]),
            None => MapperLocation::RegisterPair(*self),
         }
     }
}

impl FPRegister {
    pub(crate) fn map_to_location(
        &self,
        potential_target: Option<&MapperLocation>,
        translator: &mut Translator<'_, '_>,
        scratch_variable_map: &mut Vec<MapperLocation>,
        val_size: ValueSize,
    ) -> MapperLocation {
        match potential_target {
            Some(target) => MapperLocation::FPRegister { reg: *self, val_size}.map_to_location(target, translator, scratch_variable_map, &vec![]),
            None => MapperLocation::FPRegister { reg: *self, val_size }, // default to f32 size if no target is specified
        }
    }
}