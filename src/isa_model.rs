pub(crate) mod map_instr_to_bin;
pub(crate) mod machine_instructions;
pub(crate) mod mapping;
pub(crate) mod registers;


use alloc::vec::Vec;
use alloc::vec;
use defmt::Format;
use wasmparser::ValType;
pub(crate) use machine_instructions::{RiscvInstr, Const12, Const20};
pub(crate) use map_instr_to_bin::InstructionFormat;
pub(crate) use registers::{GPRegister, FPRegister, RegisterPair, Register};



// Register definitions do not follow the standard ABI
// x0 is hardwired zero, always reads as 0 and writes to it are ignored
#[allow(unused)] // x1 is the return address register, we never call it but we document it here
pub(crate) const RETURN_ADDRESS: GPRegister = GPRegister(1); // standard convention, assumed for compact instructions
pub(crate) const STACK_POINTER: GPRegister = GPRegister(2); // standard convention
pub(crate) const FRAME_POINTER: GPRegister = GPRegister(3); // convention uses x8 for frame pointer, but we use x3 to make it easier to encode compact instructions that use x8-x15
pub(crate) const GLOBAL_BASE: GPRegister = GPRegister(4); // base address for wasm globals
pub(crate) const LINEAR_MEMORY_BASE: GPRegister = GPRegister(5); // base address for wasm linear memory
pub(crate) const STACK_BASE: GPRegister = GPRegister(6); // stack pointer at the beginning of the function
pub(crate) const TABLE_BASE: GPRegister = GPRegister(7); // base address for wasm tables

pub(crate) const MIN_ALLOCATABLE_GP_REGISTER: u8 = 8; // first allocatable GP register for the mapper


#[cfg(feature = "address-masking")]
pub(crate) const ADDRESS_MASK: GPRegister = GPRegister(26); // register that holds the address mask for bounds checking

pub(crate) const MAX_GP_REGISTERS: usize = 32;
pub(crate) const MAX_FP_REGISTERS: usize = 32;

// x26 is reserved for potential address masking.
// x27-x30 are reserved for helper call ABI argument registers.
// x31 is used to save the stack pointer in WasmRuntime::call_function_asm

#[cfg(feature = "address-masking")]
pub(crate) const MAX_USABLE_GP_REGISTERS: usize = 26;
#[cfg(not(feature = "address-masking"))]
pub(crate) const MAX_USABLE_GP_REGISTERS: usize = 27;
// We can only use GP registers x8-x15 for compact instructions, so we try to use these as much as possible
// Then we use x16-x26




#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MapperLocation {
    GPRegister(GPRegister),
    RegisterPair(RegisterPair), // for 64-bit values that need to be stored in two registers
    FPRegister{reg:FPRegister, val_size: ValueSize}, // need to know if it's single or double precision
    Frame{offset:i16, val_type:ValueType},
    Stack{offset:i32, val_type:ValueType},
    Immediate(Immediate),
    Global{offset:u32, val_type:ValueType},
}

impl MapperLocation {
    pub(crate) fn get_type(&self) -> ValueType {
        match self {
            MapperLocation::GPRegister(_) => ValueType::I32,
            MapperLocation::RegisterPair(_) => ValueType::I64,
            MapperLocation::FPRegister{val_size, ..} => match val_size {
                ValueSize::Word => ValueType::F32,
                ValueSize::DoubleWord => ValueType::F64,
            },
            MapperLocation::Frame { val_type, .. } => *val_type,
            MapperLocation::Stack { val_type, .. } => *val_type,
            MapperLocation::Immediate(imm) => match imm {
                Immediate::I32(_) => ValueType::I32,
                Immediate::I64(_) => ValueType::I64,
                Immediate::F32(_) => ValueType::F32,
                Immediate::F64(_) => ValueType::F64,
            },
            MapperLocation::Global { val_type, .. } => *val_type,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Format)]
/// Represents a WebAssembly value type, used for type checking and determining how to handle values in code generation.
pub enum ValueType {
    I32,
    I64,
    F32,
    F64,
}

impl ValueType {
    pub(crate) fn from_wasmparser(valtype: &ValType) -> Self {
        match valtype {
            ValType::I32 => ValueType::I32,
            ValType::I64 => ValueType::I64,
            ValType::F32 => ValueType::F32,
            ValType::F64 => ValueType::F64,
            _ => panic!("Unsupported value type")
        }
    }

    pub(crate) fn as_valuesize(&self) -> ValueSize {
        match self {
            ValueType::I32 | ValueType::F32 => ValueSize::Word,
            ValueType::I64 | ValueType::F64 => ValueSize::DoubleWord,
        }
    }
    
    pub(crate) fn as_bytes(&self) -> usize {
        match self {
            ValueType::I32 | ValueType::F32 => 4,
            ValueType::I64 | ValueType::F64 => 8,
        }
    }

    pub(crate) fn half_size(&self) -> ValueType {
        match self {
            ValueType::I32 | ValueType::I64 => ValueType::I32,
            ValueType::F32 | ValueType::F64 => ValueType::F32,
        }
    }

}


#[derive(Copy, Clone, PartialEq, Debug, Format)]
pub enum ValueSize {
    Word,
    DoubleWord,
}

impl ValueSize {
    pub fn as_bytes(&self) -> usize {
        match self {
            ValueSize::Word => 4,
            ValueSize::DoubleWord => 8
        }
    }

    pub fn from_valtype(valtype: &ValType) -> Self {
        match valtype {
            ValType::I32 | ValType::F32 => ValueSize::Word,
            ValType::I64 | ValType::F64 => ValueSize::DoubleWord,
            _ => panic!("Unsupported value type")
        }
    }
}

#[derive(Clone, PartialEq)]
pub (crate) enum Memsize {
    Byte,
    HalfWord,
    Word,
    DoubleWord,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Immediate {
    I32(u32),
    I64(u64),
    F32(u32), // store the bit pattern of the float as an integer
    F64(u64), // store the bit pattern of the float as an integer
}

impl Immediate {
    pub fn as_u32(&self) -> u32 {
        match self {
            Immediate::I32(imm) => *imm,
            Immediate::F32(imm) => *imm,
            _ => panic!("Expected u32 or f32, got {:?}", self)
        }
    }
    
    pub fn as_u64(&self) -> u64 {
        match self {
            Immediate::I64(imm) => *imm,
            Immediate::F64(imm) => *imm,
            _ => panic!("Expected u64 or f64, got {:?}", self)
        }
    } 

    pub(crate) fn lower_half(&self) -> Immediate {
        match self {
            Immediate::I32(_) => *self,
            Immediate::I64(imm) => Immediate::I32(*imm as u32),
            Immediate::F32(_) => *self,
            Immediate::F64(imm) => Immediate::F32(*imm as u32),
        }
    }
    
    pub fn as_word_vector(&self) -> Vec<u32> {
        match self {
            Immediate::I32(imm) => vec![*imm],
            Immediate::I64(imm) => vec![ (*imm >> 32) as u32, *imm as u32],
            Immediate::F32(imm) => vec![*imm],
            Immediate::F64(imm) => vec![ (*imm >> 32) as u32, *imm as u32],
        }
    }
}


