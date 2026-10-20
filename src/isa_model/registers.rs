use defmt::Format;

use crate::isa_model::{Const12, RiscvInstr, ValueSize};
use crate::translator::Translator;



#[derive(Copy,Clone, PartialEq, Debug, Format)]
/// General Purpose registers, 32 of them
pub(crate) struct GPRegister(pub u8);

impl GPRegister {
    pub(crate) fn new(register:u8) -> Self {
        GPRegister(register & 0x1F)
    }

    /// Move the value in this register to the target register, if they are not the same.
    pub(crate) fn move_to(self, target: GPRegister, translator: &mut Translator<'_, '_>) {
        if self == target {
            return;
        }

        translator.push_instruction(RiscvInstr::ADDI {
            rd: target,
            rs1: self,
            imm: Const12::new(0),
        });
    }
}

#[derive(Copy,Clone, PartialEq, Debug, Format)]
pub(crate) struct RegisterPair {
    pub low: GPRegister,
    pub high: GPRegister,
}

impl RegisterPair {
    pub(crate) fn new(low: u8, high: u8) -> Self {
        RegisterPair {
            low: GPRegister::new(low),
            high: GPRegister::new(high),
        }
    }

    pub(crate) fn low(self) -> GPRegister {
        self.low
    }

    pub(crate) fn high(self) -> GPRegister {
        self.high
    }
}



#[derive(Copy,Clone, PartialEq, Debug, Format)]
/// Floating Point registers that can be used for either single or double precision, 32 of them
pub(crate) struct FPRegister(pub u8);

impl FPRegister {
    pub(crate) fn new(register:u8) -> Self {
        FPRegister(register & 0x1F)
    }

    pub(crate) fn move_to(self, target: FPRegister, val_size: ValueSize, translator: &mut Translator<'_, '_>) {
        if self == target {
            return;
        }

        translator.push_instruction(match val_size {
            ValueSize::Word => RiscvInstr::FSGNJS {
                rd: target,
                rs1: self,
                rs2: self,
            },
            ValueSize::DoubleWord => RiscvInstr::FSGNJD {
                rd: target,
                rs1: self,
                rs2: self,
            },
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Format)]
pub(crate) enum Register {
    GP(GPRegister),
    FP(FPRegister),
    Pair(RegisterPair),
}