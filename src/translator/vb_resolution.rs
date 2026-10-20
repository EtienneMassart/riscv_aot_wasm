#![forbid(unsafe_code)]

//! # VB (Valent Block) Resolution Engine
//!
//! This module implements the core VB resolution logic that converts virtual WebAssembly operand
//! stack expressions into concrete machine code for the Aurix processor.
//!
//! ## Key Concepts
//!
//! - **VB (Valent Block)**: An intermediate representation (IR) for WebAssembly operand stack expressions
//! - **VB Resolution**: The process of converting VB expressions into machine instructions
//! - **Scratch Variables**: Temporary registers and memory locations used during code generation
//! - **Target Mapping**: Specifying where the result of an operation should be placed
//!
//! ## VB Types
//!
//! - **AtomicVB**: Leaf nodes representing constants, locals, globals, and resolved values
//! - **UnaryVB**: Single-operand operations (negation, conversion, loads, etc.)
//! - **BinaryVB**: Two-operand operations (arithmetic, comparison, bitwise, etc.)
//! - **Select**: Conditional selection between two values, the only ternary operation
//!
//! ## Resolution Process
//!
//! 1. **Post-order DFS traversal**: Process child nodes before parent nodes
//! 2. **Register allocation**: Assign scratch registers for intermediate results
//! 3. **Instruction generation**: Emit appropriate machine instructions
//! 4. **Target mapping**: Place final result in the specified location
//!
//! ## Key Functions
//!
//! - `resolve_all()`: Resolves all VBs on the stack to concrete stack locations
//! - `resolve_with_target()`: Resolves a single VB with an optional target location
//! - `dispatch_*()`: Handles resolution for different VB types
//! - `gen_*()`: Generates machine code for specific operations

use alloc::vec;
use alloc::vec::Vec;
use crate::isa_model::{Const12, Const20, GPRegister, Immediate, MapperLocation, RegisterPair, ValueSize, ValueType, LINEAR_MEMORY_BASE, STACK_POINTER};
#[cfg(feature = "address-masking")]
use crate::isa_model::ADDRESS_MASK;
use crate::isa_model::machine_instructions::{RiscvInstr, RoundingMode, ShAmt5};
use crate::isa_model::mapping::LocationCouple;
use crate::translator::Translator;
use crate::vb::{AtomicVB, BinaryVB, UnaryVB, VB};
use super::library_function::{LibraryFunction, Operands};
use super::StackHeight;

// for gen_i32_simple_binary_op
enum I32BinaryInstr { // TODO: all can be optimized for immediates except MUL, DIV and REM. Compressed instructions are also available
    ADD,    
    SUB,    
    XOR,
    OR,
    AND,
    MUL,
    EQ,
    NE,
    DIVS,
    DIVU,
    REMS,
    REMU,
    SHL,    
    SHRS,   
    SHRU,   
    ROTL,   
    ROTR,   
    LTS,    
    LTU,    
    GES,    
    GEU,    
    GTS,    
    GTU,    
    LES,    
    LEU,    
}

// for gen_single_operand_op
enum I32UnaryInstr {
    CPOP,
    CLZ,
    CTZ,
    EQZ,
}

enum FPUnaryInstr {
    ABS,
    NEG,
    SQRT,
}

enum FPBinaryInstr {
    ADD,
    SUB,
    MUL,
    DIV,
    COPYSIGN,
}

// cannot be merged with binary instr because the result is a GP register and not a float
enum FPCompInstr {
    EQ,
    NE,
    LT,
    LE,
    GT,
    GE,
}


impl Translator<'_, '_> {

    // ================================================================================
    // CORE VB RESOLUTION FUNCTIONS
    // ================================================================================

    /// Resolves all VB expressions on the stack to concrete stack locations.
    ///
    /// This function processes the entire VB stack, converting each virtual expression
    /// into machine code and updating stack offsets accordingly. Already-resolved VBs
    /// are skipped to avoid redundant work.
    ///
    /// ## Process
    /// 1. Iterate through all VBs on the stack
    /// 2. Skip already-resolved VBs (maintain stack offset tracking)
    /// 3. For unresolved VBs:
    ///    - Handle NaN canonicalization if needed
    ///    - Perform post-order DFS traversal for code generation
    ///    - Update stack offset and mark as resolved
    ///
    /// ## Stack Layout
    /// The runtime stack grows downward with each resolved VB consuming space
    /// based on its value size (4 bytes for Word, 8 bytes for DoubleWord).
    pub(crate) fn resolve_all(&mut self) {
        let mut stack_offset = StackHeight::from(0);

        for index in 0..self.vb_stack.len() {
            let vb = self.vb_stack[index].clone();
            match vb {
                VB::AtomicVB(AtomicVB::Resolved { offset, .. }) => {
                    // Already resolved - just update our stack offset tracking
                    stack_offset = offset;
                },
                _ => {
                    let val_type = vb.val_type(&self.locals_map, &self.global_translator.globals_map);
                    let mut scratch_variable_map = Vec::new();
                    let target_stack_offset = stack_offset.add(val_type.as_bytes());
                    self.modeled_stack_height = target_stack_offset;
                    self.maybe_rebase_stack_pointer();
                    let stack_location = MapperLocation::Stack {
                        offset: target_stack_offset.to_i32(),
                        val_type,
                    };

                    // Perform post-order DFS to generate machine code
                    vb.post_order_dfs(|vb, is_top| {
                        self.resolve_vb(if is_top { Some(&stack_location) } else { None }, vb, &mut scratch_variable_map);
                    });

                    // Update stack offset and mark as resolved
                    stack_offset = target_stack_offset;
                    self.vb_stack[index] = VB::AtomicVB(AtomicVB::Resolved { val_type, offset: stack_offset.into() });
                }
            }
        }

        self.modeled_stack_height = stack_offset;
        self.maybe_rebase_stack_pointer();
    }

    /// Resolves a single VB expression from the top of the stack with an optional target location.
    ///
    /// This is the primary function for converting a VB expression into machine code when
    /// you need the result in a specific location (register, memory, etc.) or want to
    /// determine where the result ended up.
    ///
    /// ## Parameters
    /// - `target`: Optional target location where the result should be placed
    ///   - `Some(location)`: Force result to specific location (register, memory, etc.)
    ///   - `None`: Let the resolution choose the most efficient location
    ///
    /// ## Returns
    /// The `MapperLocation` where the result was actually placed. This may differ
    /// from the requested target if optimizations were applied.
    ///
    /// ## Process
    /// 1. Pop the top VB from the stack
    /// 2. Handle NaN canonicalization if needed
    /// 3. Perform post-order DFS traversal
    /// 4. Return the final result location
    pub(crate) fn resolve_with_target(&mut self, target: Option<&MapperLocation>) -> MapperLocation {
        let vb = self.pop_vb_stack();
        let mut scratch_variable_map: Vec<MapperLocation> = Vec::new();
        vb.post_order_dfs(|vb, is_top| self.resolve_vb(if is_top { target } else { None }, vb, &mut scratch_variable_map));
        scratch_variable_map.pop().unwrap()
    }

    // ================================================================================
    // VB DISPATCH AND RESOLUTION
    // ================================================================================

    /// Core VB resolution function that dispatches to appropriate handlers based on VB type.
    ///
    /// This function is called during the post-order DFS traversal and is responsible
    /// for generating the appropriate machine code for each VB node.
    ///
    /// ## Parameters
    /// - `potential_target`: Where the result should be placed (if specified)
    /// - `vb`: The VB expression to resolve
    /// - `scratch_variable_map`: Stack of intermediate result locations
    ///
    /// ## VB Type Dispatch
    /// - **AtomicVB**: Constants, locals, globals, resolved values
    /// - **UnaryVB**: Single-operand operations (loads, conversions, arithmetic)
    /// - **BinaryVB**: Two-operand operations (arithmetic, comparisons, bitwise)
    /// - **Select**: Conditional selection (WebAssembly select instruction)
    fn resolve_vb(&mut self, potential_target: Option<&MapperLocation>, vb: &VB, scratch_variable_map: &mut Vec<MapperLocation>) {
        let result = match vb {
            VB::AtomicVB(atomic_vb) => self.dispatch_atomic_vb(atomic_vb, potential_target, scratch_variable_map),
            VB::UnaryVB { vb, .. } => self.dispatch_unary_vb(scratch_variable_map, vb, potential_target),
            VB::BinaryVB { vb, .. } => self.dispatch_binary_vb(scratch_variable_map, vb, potential_target),
            VB::Select { val_type, .. } => self.gen_select(scratch_variable_map, potential_target, *val_type),
        };
        scratch_variable_map.push(result);
    }

    // ================================================================================
    // ATOMIC VB RESOLUTION (Constants, Variables, Resolved Values)
    // ================================================================================

    /// Resolves atomic VB expressions (leaf nodes in the expression tree).
    ///
    /// Atomic VBs represent the simplest form of values that don't require computation:
    /// - **Constants**: Immediate values (i32, i64, f32, f64)
    /// - **Local variables**: Function parameters and local variables
    /// - **Global variables**: Module-level variables
    /// - **Resolved values**: Previously computed results on the stack
    /// - **Special values**: Memory size, unreachable markers
    ///
    /// ## Target Handling
    /// If a target location is specified, the atomic value is moved/copied to that location.
    /// Otherwise, the most efficient representation is used (immediate values stay as
    /// immediates, variables reference their storage locations, etc.).
    fn dispatch_atomic_vb(&mut self, atomic_vb: &AtomicVB, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let result = match atomic_vb {
            AtomicVB::I32Const { imm } => MapperLocation::Immediate(Immediate::I32(*imm as u32)),
            AtomicVB::I64Const { imm } => MapperLocation::Immediate(Immediate::I64(*imm as u64)),
            AtomicVB::F32Const { imm } => MapperLocation::Immediate(Immediate::F32(*imm)),
            AtomicVB::F64Const { imm } => MapperLocation::Immediate(Immediate::F64(*imm)),
            AtomicVB::Local { index } => self.locals_map[*index as usize].clone(),
            AtomicVB::Global { index } => {
                let (offset, val_type) = self.global_translator.globals_map[*index as usize];
                MapperLocation::Global { offset, val_type }
            },
            AtomicVB::Resolved { val_type, offset } => MapperLocation::Stack {
                offset: offset.to_i32(),
                val_type: *val_type,
            },
            AtomicVB::MemorySize => MapperLocation::Global { offset: 0, val_type: ValueType::I32 }, // Memory size is stored in global space at offset 0
        };
        match potential_target {
            Some(target) => result.map_to_location(target, self, scratch_variable_map, &vec![]),
            None => result
        }
    }

    // ================================================================================
    // UNARY VB RESOLUTION (Single-Operand Operations)
    // ================================================================================

    /// Resolves unary VB expressions (operations with a single operand).
    ///
    /// Unary operations include:
    /// - **Arithmetic**: Negation, absolute value, square root
    /// - **Bitwise**: Count leading/trailing zeros, population count
    /// - **Conversions**: Type conversions between integers and floats
    /// - **Comparisons**: Zero comparisons (eqz)
    /// - **Memory loads**: All load operations with different sizes and signedness
    /// - **Math functions**: Ceiling, floor, truncate, nearest (often library calls)
    ///
    /// ## Process
    /// 1. Pop the child operand from the scratch variable stack
    /// 2. Dispatch to the appropriate generator function based on operation type
    /// 3. Many operations are implemented as library function calls for complex operations
    /// 4. Simple operations generate direct machine instructions
    fn dispatch_unary_vb(&mut self, scratch_variable_map: &mut Vec<MapperLocation>, vb: &UnaryVB, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child = scratch_variable_map.pop().unwrap();
        match vb {
            // Integer unary operations
            UnaryVB::I32Clz => self.gen_i32_clz(&child, scratch_variable_map, potential_target),
            UnaryVB::I32Ctz => self.gen_i32_ctz(&child, scratch_variable_map, potential_target),
            UnaryVB::I32PopCnt => self.gen_i32_popcnt(&child, scratch_variable_map, potential_target),
            UnaryVB::I32EqZ => self.gen_i32_eqz(&child, scratch_variable_map, potential_target),
            UnaryVB::I64Clz => self.gen_i64_clz(&child, scratch_variable_map, potential_target),
            UnaryVB::I64Ctz => self.gen_i64_ctz(&child, scratch_variable_map, potential_target),
            UnaryVB::I64PopCnt => self.gen_i64_popcnt(&child, scratch_variable_map, potential_target),
            UnaryVB::I64EqZ => self.gen_i64_eqz(&child, scratch_variable_map, potential_target),

            // Floating-point unary operations
            UnaryVB::F32Abs => self.gen_f32_abs(&child, scratch_variable_map, potential_target),
            UnaryVB::F32Neg => self.gen_f32_neg(&child, scratch_variable_map, potential_target),
            UnaryVB::F32Sqrt => self.gen_f32_sqrt(&child, scratch_variable_map, potential_target),
            UnaryVB::F32Ceil => self.call_library_function(potential_target, LibraryFunction::F32Ceil, Operands::new_single(&child), ValueSize::Word, ValueType::F32, scratch_variable_map),
            UnaryVB::F32Floor => self.call_library_function(potential_target, LibraryFunction::F32Floor, Operands::new_single(&child), ValueSize::Word, ValueType::F32, scratch_variable_map),
            UnaryVB::F32Trunc => self.call_library_function(potential_target, LibraryFunction::F32Trunc, Operands::new_single(&child), ValueSize::Word, ValueType::F32, scratch_variable_map),
            UnaryVB::F32Nearest => self.call_library_function(potential_target, LibraryFunction::F32Nearest, Operands::new_single(&child), ValueSize::Word, ValueType::F32, scratch_variable_map),
            UnaryVB::F64Abs => self.gen_f64_abs(&child, scratch_variable_map, potential_target),
            UnaryVB::F64Neg => self.gen_f64_neg(&child, scratch_variable_map, potential_target),
            UnaryVB::F64Sqrt => self.gen_f64_sqrt(&child, scratch_variable_map, potential_target),
            UnaryVB::F64Ceil => self.call_library_function(potential_target, LibraryFunction::F64Ceil, Operands::new_single(&child), ValueSize::DoubleWord, ValueType::F64, scratch_variable_map),
            UnaryVB::F64Floor => self.call_library_function(potential_target, LibraryFunction::F64Floor, Operands::new_single(&child), ValueSize::DoubleWord, ValueType::F64, scratch_variable_map),
            UnaryVB::F64Trunc => self.call_library_function(potential_target, LibraryFunction::F64Trunc, Operands::new_single(&child), ValueSize::DoubleWord, ValueType::F64, scratch_variable_map),
            UnaryVB::F64Nearest => self.call_library_function(potential_target, LibraryFunction::F64Nearest, Operands::new_single(&child), ValueSize::DoubleWord, ValueType::F64, scratch_variable_map),

            // Integer conversions
            UnaryVB::I32WrapI64 => self.gen_i32_wrap_i64(&child, scratch_variable_map, potential_target),
            UnaryVB::I32TruncF32S => self.gen_i32_trunc_f32s(&child, scratch_variable_map, potential_target),
            UnaryVB::I32TruncF32U => self.gen_i32_trunc_f32u(&child, scratch_variable_map, potential_target),
            UnaryVB::I32TruncF64S => self.gen_i32_trunc_f64s(&child, scratch_variable_map, potential_target),
            UnaryVB::I32TruncF64U => self.gen_i32_trunc_f64u(&child, scratch_variable_map, potential_target),
            UnaryVB::I64ExtendI32U => self.gen_i64_extend_i32u(potential_target, scratch_variable_map, &child),
            UnaryVB::I64ExtendI32S => self.gen_i64_extend_i32s(potential_target, &child, scratch_variable_map),
            UnaryVB::I64TruncF32S => self.call_library_function(potential_target, LibraryFunction::I64TruncF32S, Operands::new_single(&child), ValueSize::Word, ValueType::I64, scratch_variable_map),
            UnaryVB::I64TruncF32U => self.call_library_function(potential_target, LibraryFunction::I64TruncF32U, Operands::new_single(&child), ValueSize::Word, ValueType::I64, scratch_variable_map),
            UnaryVB::I64TruncF64S => self.call_library_function(potential_target, LibraryFunction::I64TruncF64S, Operands::new_single(&child), ValueSize::DoubleWord, ValueType::I64, scratch_variable_map),
            UnaryVB::I64TruncF64U => self.call_library_function(potential_target, LibraryFunction::I64TruncF64U, Operands::new_single(&child), ValueSize::DoubleWord, ValueType::I64, scratch_variable_map),

            // Floating-point conversions
            UnaryVB::F32ConvertI32S => self.gen_f32_convert_i32s(&child, scratch_variable_map, potential_target),
            UnaryVB::F32ConvertI32U => self.gen_f32_convert_i32u(&child, scratch_variable_map, potential_target),
            UnaryVB::F32ConvertI64S => self.call_library_function(potential_target, LibraryFunction::F32ConvertI64S, Operands::new_single(&child), ValueSize::DoubleWord, ValueType::F32, scratch_variable_map),
            UnaryVB::F32ConvertI64U => self.call_library_function(potential_target, LibraryFunction::F32ConvertI64U, Operands::new_single(&child), ValueSize::DoubleWord, ValueType::F32, scratch_variable_map),
            UnaryVB::F64ConvertI32S => self.gen_f64_convert_i32s(&child, scratch_variable_map, potential_target),
            UnaryVB::F64ConvertI32U => self.gen_f64_convert_i32u(&child, scratch_variable_map, potential_target),
            UnaryVB::F64ConvertI64S => self.call_library_function(potential_target, LibraryFunction::F64ConvertI64S, Operands::new_single(&child), ValueSize::DoubleWord, ValueType::F64, scratch_variable_map),
            UnaryVB::F64ConvertI64U => self.call_library_function(potential_target, LibraryFunction::F64ConvertI64U, Operands::new_single(&child), ValueSize::DoubleWord, ValueType::F64, scratch_variable_map),
            UnaryVB::F32DemoteF64 => self.gen_f32_demote_f64(&child, scratch_variable_map, potential_target),
            UnaryVB::F64PromoteF32 => self.gen_f64_promote_f32(&child, scratch_variable_map, potential_target),

            // Reinterpret operations
            UnaryVB::I32ReinterpretF32 => self.gen_i32_reinterpret_f32(&child, scratch_variable_map, potential_target),
            UnaryVB::I64ReinterpretF64 => self.gen_i64_reinterpret_f64(&child, scratch_variable_map, potential_target),
            UnaryVB::F32ReinterpretI32 => self.gen_f32_reinterpret_i32(&child, scratch_variable_map, potential_target),
            UnaryVB::F64ReinterpretI64 => self.gen_f64_reinterpret_i64(&child, scratch_variable_map, potential_target),

            // Memory loads
            UnaryVB::I32Load { offset, align } => self.gen_i32_load(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::I64Load { offset, align } => self.gen_i64_load(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::F32Load { offset, align } => self.gen_f32_load(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::F64Load { offset, align } => self.gen_f64_load(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::I32Load8s { offset, align } => self.gen_i32_load8s(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::I32Load8u { offset, align } => self.gen_i32_load8u(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::I32Load16s { offset, align } => self.gen_i32_load16s(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::I32Load16u { offset, align } => self.gen_i32_load16u(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::I64Load8s { offset, align } => self.gen_i64_load8s(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::I64Load8u { offset, align } => self.gen_i64_load8u(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::I64Load16s { offset, align } => self.gen_i64_load16s(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::I64Load16u { offset, align } => self.gen_i64_load16u(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::I64Load32s { offset, align } => self.gen_i64_load32s(&child, *offset, *align, potential_target, scratch_variable_map),
            UnaryVB::I64Load32u { offset, align } => self.gen_i64_load32u(&child, *offset, *align, potential_target, scratch_variable_map),
        }
    }

    // ================================================================================
    // BINARY VB RESOLUTION (Two-Operand Operations)
    // ================================================================================

    /// Resolves binary VB expressions (operations with two operands).
    ///
    /// Binary operations include:
    /// - **Arithmetic**: Addition, subtraction, multiplication, division, remainder
    /// - **Comparisons**: Equality, inequality, less than, greater than, etc.
    /// - **Bitwise**: AND, OR, XOR, shifts, rotations
    /// - **Floating-point**: All floating-point arithmetic and comparisons
    ///
    /// ## Process
    /// 1. Pop the right-hand operand (RHS) from scratch variable stack
    /// 2. Pop the left-hand operand (LHS) from scratch variable stack
    /// 3. Dispatch to appropriate generator based on operation type
    /// 4. Complex operations (64-bit, floating-point) often use library calls
    /// 5. Simple operations generate direct machine instructions
    ///
    /// ## Operand Order
    /// Note that operands are popped in reverse order (RHS first, then LHS) due to
    /// stack-based evaluation order in the post-order traversal.
    fn dispatch_binary_vb(&mut self, scratch_variable_map: &mut Vec<MapperLocation>, vb: &BinaryVB, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let rhs = scratch_variable_map.pop().unwrap();
        let lhs = scratch_variable_map.pop().unwrap();
        match vb {
            // i32 arithmetic
            BinaryVB::I32Add => self.gen_i32_add(potential_target, scratch_variable_map, &lhs, &rhs),
            BinaryVB::I32Sub => self.gen_i32_sub(potential_target, scratch_variable_map, &lhs, &rhs),
            BinaryVB::I32Mul => self.gen_i32_mul(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32DivS => self.gen_i32_div_s(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32DivU => self.gen_i32_div_u(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32RemS => self.gen_i32_rem_s(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32RemU => self.gen_i32_rem_u(&lhs, &rhs, scratch_variable_map, potential_target),

            // i32 bitwise and shifts
            BinaryVB::I32And => self.gen_i32_and(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32Or => self.gen_i32_or(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32Xor => self.gen_i32_xor(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32Shl => self.gen_i32_shl(&rhs, scratch_variable_map, &lhs, potential_target),
            BinaryVB::I32ShrS => self.gen_i32_shr_s(&rhs, scratch_variable_map, &lhs, potential_target),
            BinaryVB::I32ShrU => self.gen_i32_shr_u(&rhs, scratch_variable_map, &lhs, potential_target),
            BinaryVB::I32Rotl => self.gen_i32_rotl(&rhs, scratch_variable_map, &lhs, potential_target),
            BinaryVB::I32Rotr => self.gen_i32_rotr(&rhs, scratch_variable_map, &lhs, potential_target),

            // i32 comparisons
            BinaryVB::I32Eq => self.gen_i32_eq(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32Ne => self.gen_i32_ne(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32LtS => self.gen_i32_lts(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32LtU => self.gen_i32_ltu(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32GeS => self.gen_i32_ges(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32GeU => self.gen_i32_geu(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32LeS => self.gen_i32_les(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32LeU => self.gen_i32_leu(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32GtS => self.gen_i32_gts(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I32GtU => self.gen_i32_gtu(&lhs, &rhs, scratch_variable_map, potential_target),

            // i64 arithmetic
            BinaryVB::I64Add => self.gen_i64_add(potential_target, scratch_variable_map, &lhs, &rhs),
            BinaryVB::I64Sub => self.gen_i64_sub(potential_target, scratch_variable_map, &lhs, &rhs),
            BinaryVB::I64Mul => self.gen_i64_mul(potential_target, scratch_variable_map, &lhs, &rhs),
            BinaryVB::I64DivS => self.call_library_function(potential_target, LibraryFunction::I64DivS, Operands::new_double(&lhs, &rhs), ValueSize::DoubleWord, ValueType::I64, scratch_variable_map),
            BinaryVB::I64DivU => self.call_library_function(potential_target, LibraryFunction::I64DivU, Operands::new_double(&lhs, &rhs), ValueSize::DoubleWord, ValueType::I64, scratch_variable_map),
            BinaryVB::I64RemS => self.call_library_function(potential_target, LibraryFunction::I64RemS, Operands::new_double(&lhs, &rhs), ValueSize::DoubleWord, ValueType::I64, scratch_variable_map),
            BinaryVB::I64RemU => self.call_library_function(potential_target, LibraryFunction::I64RemU, Operands::new_double(&lhs, &rhs), ValueSize::DoubleWord, ValueType::I64, scratch_variable_map),

            // i64 bitwise and shifts
            BinaryVB::I64And => self.gen_i64_and(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64Or => self.gen_i64_or(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64Xor => self.gen_i64_xor(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64Shl => self.call_library_function(potential_target, LibraryFunction::I64Shl, Operands::new_double(&lhs, &rhs), ValueSize::DoubleWord, ValueType::I64, scratch_variable_map),
            BinaryVB::I64ShrS => self.call_library_function(potential_target, LibraryFunction::I64ShrS, Operands::new_double(&lhs, &rhs), ValueSize::DoubleWord, ValueType::I64, scratch_variable_map),
            BinaryVB::I64ShrU => self.call_library_function(potential_target, LibraryFunction::I64ShrU, Operands::new_double(&lhs, &rhs), ValueSize::DoubleWord, ValueType::I64, scratch_variable_map),
            BinaryVB::I64Rotl => self.call_library_function(potential_target, LibraryFunction::I64Rotl, Operands::new_double(&lhs, &rhs), ValueSize::DoubleWord, ValueType::I64, scratch_variable_map),
            BinaryVB::I64Rotr => self.call_library_function(potential_target, LibraryFunction::I64Rotr, Operands::new_double(&lhs, &rhs), ValueSize::DoubleWord, ValueType::I64, scratch_variable_map),

            // i64 comparisons
            BinaryVB::I64Eq => self.gen_i64_eq(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64Ne => self.gen_i64_ne(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64LtS => self.gen_i64_lts(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64LtU => self.gen_i64_ltu(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64GeS => self.gen_i64_ges(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64GeU => self.gen_i64_geu(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64LeS => self.gen_i64_les(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64LeU => self.gen_i64_leu(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64GtS => self.gen_i64_gts(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::I64GtU => self.gen_i64_gtu(&lhs, &rhs, scratch_variable_map, potential_target),

            // f32 binary operations
            BinaryVB::F32Add => self.gen_f32_add(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F32Sub => self.gen_f32_sub(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F32Mul => self.gen_f32_mul(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F32Div => self.gen_f32_div(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F32Min => self.call_library_function(potential_target, LibraryFunction::F32Min, Operands::new_double(&lhs, &rhs), ValueSize::Word, ValueType::F32, scratch_variable_map),
            BinaryVB::F32Max => self.call_library_function(potential_target, LibraryFunction::F32Max, Operands::new_double(&lhs, &rhs), ValueSize::Word, ValueType::F32, scratch_variable_map),
            BinaryVB::F32CopySign => self.gen_f32_copysign(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F32Eq => self.gen_f32_eq(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F32Ne => self.gen_f32_ne(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F32Lt => self.gen_f32_lt(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F32Ge => self.gen_f32_ge(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F32Gt => self.gen_f32_gt(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F32Le => self.gen_f32_le(&lhs, &rhs, scratch_variable_map, potential_target),

            // f64 binary operations
            BinaryVB::F64Add => self.gen_f64_add(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F64Sub => self.gen_f64_sub(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F64Mul => self.gen_f64_mul(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F64Div => self.gen_f64_div(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F64Min => self.call_library_function(potential_target, LibraryFunction::F64Min, Operands::new_double(&lhs, &rhs), ValueSize::DoubleWord, ValueType::F64, scratch_variable_map),
            BinaryVB::F64Max => self.call_library_function(potential_target, LibraryFunction::F64Max, Operands::new_double(&lhs, &rhs), ValueSize::DoubleWord, ValueType::F64, scratch_variable_map),
            BinaryVB::F64CopySign => self.gen_f64_copysign(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F64Eq => self.gen_f64_eq(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F64Ne => self.gen_f64_ne(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F64Lt => self.gen_f64_lt(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F64Ge => self.gen_f64_ge(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F64Gt => self.gen_f64_gt(&lhs, &rhs, scratch_variable_map, potential_target),
            BinaryVB::F64Le => self.gen_f64_le(&lhs, &rhs, scratch_variable_map, potential_target),
        }
    }

    // ================================================================================
    // OPERATION GENERATORS (Machine Code Generation)
    // ================================================================================

    /// Generates machine code for the WebAssembly `select` instruction.
    ///
    /// Semantics: `select(lhs, rhs, selector)` returns `lhs` when selector != 0,
    /// otherwise returns `rhs`.
    ///
    /// RISC-V RV32 has no generic select instruction, so we lower to:
    /// 1. `beq selector, x0, rhs_path`
    /// 2. move lhs into destination
    /// 3. `jal x0, end`
    /// 4. rhs_path: move rhs into destination
    fn gen_select(&mut self, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>, val_type: ValueType) -> MapperLocation {
        let selector = scratch_variable_map.pop().unwrap();
        let rhs = scratch_variable_map.pop().unwrap();
        let lhs = scratch_variable_map.pop().unwrap();

        match val_type {
            ValueType::I32 => {
                let selector_reg = selector.map_to_gp_register(None, self, scratch_variable_map, &vec![lhs.clone(), rhs.clone()]);
                let dest = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![MapperLocation::GPRegister(selector_reg)]);
                let rhs_reg = rhs.map_to_gp_register(None, self, scratch_variable_map, &vec![MapperLocation::GPRegister(dest), MapperLocation::GPRegister(selector_reg)]);
                let lhs_reg = lhs.map_to_gp_register(None, self, scratch_variable_map, &vec![MapperLocation::GPRegister(dest), MapperLocation::GPRegister(rhs_reg), MapperLocation::GPRegister(selector_reg)]);

                self.push_instruction(RiscvInstr::BEQ { rs1: selector_reg, rs2: GPRegister(0), imm: Const12::new(12)});
                self.push_instruction_no_compress(RiscvInstr::ADDI { rd: dest, rs1: lhs_reg, imm: Const12::new(0)});
                self.push_instruction_no_compress(RiscvInstr::JAL { rd: GPRegister(0), imm: Const20::new(8)});
                self.push_instruction_no_compress(RiscvInstr::ADDI { rd: dest, rs1: rhs_reg, imm: Const12::new(0)});

                dest.map_to_location(potential_target, self, scratch_variable_map)
            }
            ValueType::I64 => {
                let selector_reg = selector.map_to_gp_register(None, self, scratch_variable_map, &vec![lhs.clone(), rhs.clone()]);
                let dest = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![MapperLocation::GPRegister(selector_reg)]);
                let rhs_pair = rhs.map_to_register_pair(None, self, scratch_variable_map, &vec![MapperLocation::RegisterPair(dest), MapperLocation::GPRegister(selector_reg)]);
                let lhs_pair = lhs.map_to_register_pair(None, self, scratch_variable_map, &vec![MapperLocation::RegisterPair(dest), MapperLocation::RegisterPair(rhs_pair), MapperLocation::GPRegister(selector_reg)]);

                self.push_instruction(RiscvInstr::BEQ { rs1: selector_reg, rs2: GPRegister(0), imm: Const12::new(16) });
                self.push_instruction_no_compress(RiscvInstr::ADDI { rd: dest.low(), rs1: lhs_pair.low(), imm: Const12::new(0) });
                self.push_instruction_no_compress(RiscvInstr::ADDI { rd: dest.high(), rs1: lhs_pair.high(), imm: Const12::new(0) });
                self.push_instruction_no_compress(RiscvInstr::JAL { rd: GPRegister(0), imm: Const20::new(8) });
                self.push_instruction_no_compress(RiscvInstr::ADDI { rd: dest.low(), rs1: rhs_pair.low(), imm: Const12::new(0) });
                self.push_instruction_no_compress(RiscvInstr::ADDI { rd: dest.high(), rs1: rhs_pair.high(), imm: Const12::new(0) });

                dest.map_to_location(potential_target, self, scratch_variable_map)
            }
            ValueType::F32 => {
                let selector_reg = selector.map_to_gp_register(None, self, scratch_variable_map, &vec![lhs.clone(), rhs.clone()]);
                let dest = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![MapperLocation::GPRegister(selector_reg)]);
                let rhs_reg = rhs.map_to_fp_register(None, ValueSize::Word, self, scratch_variable_map, &vec![MapperLocation::FPRegister { reg: dest, val_size: ValueSize::Word }, MapperLocation::GPRegister(selector_reg)]);
                let lhs_reg = lhs.map_to_fp_register(None, ValueSize::Word, self, scratch_variable_map, &vec![MapperLocation::FPRegister { reg: dest, val_size: ValueSize::Word }, MapperLocation::FPRegister { reg: rhs_reg, val_size: ValueSize::Word }, MapperLocation::GPRegister(selector_reg)]);

                self.push_instruction(RiscvInstr::BEQ { rs1: selector_reg, rs2: GPRegister(0), imm: Const12::new(12) });
                self.push_instruction_no_compress(RiscvInstr::FSGNJS { rd: dest, rs1: lhs_reg, rs2: lhs_reg });
                self.push_instruction_no_compress(RiscvInstr::JAL { rd: GPRegister(0), imm: Const20::new(8) });
                self.push_instruction_no_compress(RiscvInstr::FSGNJS { rd: dest, rs1: rhs_reg, rs2: rhs_reg });

                dest.map_to_location(potential_target, self, scratch_variable_map, ValueSize::Word)
            }
            ValueType::F64 => {
                let selector_reg = selector.map_to_gp_register(None, self, scratch_variable_map, &vec![lhs.clone(), rhs.clone()]);
                let dest = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![MapperLocation::GPRegister(selector_reg)]);
                let rhs_reg = rhs.map_to_fp_register(None, ValueSize::DoubleWord, self, scratch_variable_map, &vec![MapperLocation::FPRegister { reg: dest, val_size: ValueSize::DoubleWord }, MapperLocation::GPRegister(selector_reg)]);
                let lhs_reg = lhs.map_to_fp_register(None, ValueSize::DoubleWord, self, scratch_variable_map, &vec![MapperLocation::FPRegister { reg: dest, val_size: ValueSize::DoubleWord }, MapperLocation::FPRegister { reg: rhs_reg, val_size: ValueSize::DoubleWord }, MapperLocation::GPRegister(selector_reg)]);

                self.push_instruction(RiscvInstr::BEQ { rs1: selector_reg, rs2: GPRegister(0), imm: Const12::new(12) });
                self.push_instruction_no_compress(RiscvInstr::FSGNJD { rd: dest, rs1: lhs_reg, rs2: lhs_reg });
                self.push_instruction_no_compress(RiscvInstr::JAL { rd: GPRegister(0), imm: Const20::new(8) });
                self.push_instruction_no_compress(RiscvInstr::FSGNJD { rd: dest, rs1: rhs_reg, rs2: rhs_reg });

                dest.map_to_location(potential_target, self, scratch_variable_map, ValueSize::DoubleWord)
            }
        }
    }

    // ================================================================================
    // FLOATING-POINT OPERATION GENERATORS
    // ================================================================================

    fn gen_f32_binary_op(&mut self, op: FPBinaryInstr, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_fp_registers(self, scratch_variable_map);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(match op {
            FPBinaryInstr::ADD => RiscvInstr::FADDS { rd: dest_register, rs1: lhs_register, rs2: rhs_register, rm: RoundingMode::RNE },
            FPBinaryInstr::SUB => RiscvInstr::FSUBS { rd: dest_register, rs1: lhs_register, rs2: rhs_register, rm: RoundingMode::RNE },
            FPBinaryInstr::MUL => RiscvInstr::FMULS { rd: dest_register, rs1: lhs_register, rs2: rhs_register, rm: RoundingMode::RNE },
            FPBinaryInstr::DIV => RiscvInstr::FDIVS { rd: dest_register, rs1: lhs_register, rs2: rhs_register, rm: RoundingMode::RNE },
            FPBinaryInstr::COPYSIGN => RiscvInstr::FSGNJS { rd: dest_register, rs1: lhs_register, rs2: rhs_register },
        });
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::Word)
    }

    fn gen_f32_add(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_binary_op(FPBinaryInstr::ADD, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f32_sub(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_binary_op(FPBinaryInstr::SUB, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f32_mul(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_binary_op(FPBinaryInstr::MUL, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f32_div(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_binary_op(FPBinaryInstr::DIV, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f32_copysign(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_binary_op(FPBinaryInstr::COPYSIGN, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f32_comp_op(&mut self, op: FPCompInstr, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_fp_registers(self, scratch_variable_map);
        let dest_register = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        match op {
            FPCompInstr::EQ => self.push_instruction(RiscvInstr::FEQS { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            FPCompInstr::NE => {
                self.push_instruction(RiscvInstr::FEQS { rd: dest_register, rs1: lhs_register, rs2: rhs_register });
                self.push_instruction(RiscvInstr::XORI { rd: dest_register, rs1: dest_register, imm: Const12::new(1) });
            }
            FPCompInstr::LT => self.push_instruction(RiscvInstr::FLTS { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            FPCompInstr::LE => self.push_instruction(RiscvInstr::FLES { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            FPCompInstr::GT => self.push_instruction(RiscvInstr::FLTS { rd: dest_register, rs1: rhs_register, rs2: lhs_register }),
            FPCompInstr::GE => self.push_instruction(RiscvInstr::FLES { rd: dest_register, rs1: rhs_register, rs2: lhs_register }),            
        };
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_f32_eq(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_comp_op(FPCompInstr::EQ, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f32_ne(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_comp_op(FPCompInstr::NE, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f32_lt(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_comp_op(FPCompInstr::LT, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f32_le(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_comp_op(FPCompInstr::LE, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f32_gt(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_comp_op(FPCompInstr::GT, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f32_ge(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_comp_op(FPCompInstr::GE, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f64_binary_op(&mut self, op: FPBinaryInstr, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_fp_registers(self, scratch_variable_map);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(match op {
            FPBinaryInstr::ADD => RiscvInstr::FADDD { rd: dest_register, rs1: lhs_register, rs2: rhs_register, rm: RoundingMode::RNE },
            FPBinaryInstr::SUB => RiscvInstr::FSUBD { rd: dest_register, rs1: lhs_register, rs2: rhs_register, rm: RoundingMode::RNE },
            FPBinaryInstr::MUL => RiscvInstr::FMULD { rd: dest_register, rs1: lhs_register, rs2: rhs_register, rm: RoundingMode::RNE },
            FPBinaryInstr::DIV => RiscvInstr::FDIVD { rd: dest_register, rs1: lhs_register, rs2: rhs_register, rm: RoundingMode::RNE },
            FPBinaryInstr::COPYSIGN => RiscvInstr::FSGNJD { rd: dest_register, rs1: lhs_register, rs2: rhs_register },
        });
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::DoubleWord)
    }
    
    fn gen_f64_add(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_binary_op(FPBinaryInstr::ADD, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f64_sub(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_binary_op(FPBinaryInstr::SUB, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f64_mul(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_binary_op(FPBinaryInstr::MUL, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f64_div(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_binary_op(FPBinaryInstr::DIV, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f64_copysign(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_binary_op(FPBinaryInstr::COPYSIGN, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f64_comp_op(&mut self, op: FPCompInstr, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_fp_registers(self, scratch_variable_map);
        let dest_register = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        match op {
            FPCompInstr::EQ => self.push_instruction(RiscvInstr::FEQD { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            FPCompInstr::NE => {
                self.push_instruction(RiscvInstr::FEQD { rd: dest_register, rs1: lhs_register, rs2: rhs_register });
                self.push_instruction(RiscvInstr::XORI { rd: dest_register, rs1: dest_register, imm: Const12::new(1) });
            }
            FPCompInstr::LT => self.push_instruction(RiscvInstr::FLTD { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            FPCompInstr::LE => self.push_instruction(RiscvInstr::FLED { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            FPCompInstr::GT => self.push_instruction(RiscvInstr::FLTD { rd: dest_register, rs1: rhs_register, rs2: lhs_register }),
            FPCompInstr::GE => self.push_instruction(RiscvInstr::FLED { rd: dest_register, rs1: rhs_register, rs2: lhs_register }),            
        };
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_f64_eq(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_comp_op(FPCompInstr::EQ, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f64_ne(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_comp_op(FPCompInstr::NE, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f64_lt(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_comp_op(FPCompInstr::LT, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f64_le(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_comp_op(FPCompInstr::LE, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f64_gt(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_comp_op(FPCompInstr::GT, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_f64_ge(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_comp_op(FPCompInstr::GE, lhs, rhs, scratch_variable_map, potential_target)
    }

    // ================================================================================
    // INTEGER BINARY OPERATION GENERATORS
    // ================================================================================

    fn gen_i32_binary_op(&mut self, instr: I32BinaryInstr,lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_gp_registers(self, scratch_variable_map);
        let dest_register = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        match instr {
            I32BinaryInstr::ADD => self.push_instruction(RiscvInstr::ADD { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::SUB => self.push_instruction(RiscvInstr::SUB { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::XOR => self.push_instruction(RiscvInstr::XOR { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::OR => self.push_instruction(RiscvInstr::OR { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::AND => self.push_instruction(RiscvInstr::AND { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::MUL => self.push_instruction(RiscvInstr::MUL { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::DIVS => self.push_instruction(RiscvInstr::DIV { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::DIVU => self.push_instruction(RiscvInstr::DIVU { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::REMS => self.push_instruction(RiscvInstr::REM { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::REMU => self.push_instruction(RiscvInstr::REMU { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            // EQ: (lhs == rhs) <=> ((lhs ^ rhs) == 0). Compute xor, then set if < 1 (unsigned).
            I32BinaryInstr::EQ => {
                self.push_instruction(RiscvInstr::XOR { rd: dest_register, rs1: lhs_register, rs2: rhs_register });
                self.push_instruction(RiscvInstr::SLTIU { rd: dest_register, rs1: dest_register, imm: Const12::new(1) });
            },
            // NE: (lhs != rhs) <=> ((lhs ^ rhs) != 0). Compute xor, then set if 0 < result (unsigned).
            I32BinaryInstr::NE => {
                self.push_instruction(RiscvInstr::XOR { rd: dest_register, rs1: lhs_register, rs2: rhs_register });
                self.push_instruction(RiscvInstr::SLTU { rd: dest_register, rs1: GPRegister(0), rs2: dest_register });
            },
            I32BinaryInstr::SHL => self.push_instruction(RiscvInstr::SLL { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::SHRS => self.push_instruction(RiscvInstr::SRA { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::SHRU => self.push_instruction(RiscvInstr::SRL { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::ROTL => self.push_instruction(RiscvInstr::ROL { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::ROTR => self.push_instruction(RiscvInstr::ROR { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::LTS => self.push_instruction(RiscvInstr::SLT { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::LTU => self.push_instruction(RiscvInstr::SLTU { rd: dest_register, rs1: lhs_register, rs2: rhs_register }),
            I32BinaryInstr::GTS => self.push_instruction(RiscvInstr::SLT { rd: dest_register, rs1: rhs_register, rs2: lhs_register }),
            I32BinaryInstr::GTU => self.push_instruction(RiscvInstr::SLTU { rd: dest_register, rs1: rhs_register, rs2: lhs_register }),
            I32BinaryInstr::GES => {
                self.push_instruction(RiscvInstr::SLT { rd: dest_register, rs1: lhs_register, rs2: rhs_register });
                self.push_instruction(RiscvInstr::XORI { rd: dest_register, rs1: dest_register, imm: Const12::new(1) });
            },
            I32BinaryInstr::GEU => {
                self.push_instruction(RiscvInstr::SLTU { rd: dest_register, rs1: lhs_register, rs2: rhs_register });
                self.push_instruction(RiscvInstr::XORI { rd: dest_register, rs1: dest_register, imm: Const12::new(1) });
            },
            I32BinaryInstr::LES => {
                self.push_instruction(RiscvInstr::SLT { rd: dest_register, rs1: rhs_register, rs2: lhs_register });
                self.push_instruction(RiscvInstr::XORI { rd: dest_register, rs1: dest_register, imm: Const12::new(1) });
            },
            I32BinaryInstr::LEU => {
                self.push_instruction(RiscvInstr::SLTU { rd: dest_register, rs1: rhs_register, rs2: lhs_register });
                self.push_instruction(RiscvInstr::XORI { rd: dest_register, rs1: dest_register, imm: Const12::new(1) });
            },

        }
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i32_add(&mut self, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>, lhs: &MapperLocation, rhs: &MapperLocation) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::ADD, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_sub(&mut self, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>, lhs: &MapperLocation, rhs: &MapperLocation) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::SUB, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_mul(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::MUL, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_div_s(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::DIVS, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_div_u(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::DIVU, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_rem_s(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::REMS, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_rem_u(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::REMU, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_and(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::AND, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_or(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::OR, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_xor(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::XOR, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_shl(&mut self, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, lhs: &MapperLocation, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::SHL, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_shr_s(&mut self, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, lhs: &MapperLocation, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::SHRS, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_shr_u(&mut self, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, lhs: &MapperLocation, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::SHRU, lhs, rhs, scratch_variable_map, potential_target)
    }


    fn gen_i32_rotl(&mut self, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, lhs: &MapperLocation, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::ROTL, lhs, rhs, scratch_variable_map, potential_target)
    }


    fn gen_i32_rotr(&mut self, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, lhs: &MapperLocation, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::ROTR, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_eq(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::EQ, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_ne(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::NE, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_lts(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::LTS, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_ltu(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::LTU, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_gts(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::GTS, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_gtu(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::GTU, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_les(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::LES, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_leu(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::LEU, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_ges(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::GES, lhs, rhs, scratch_variable_map, potential_target)
    }

    fn gen_i32_geu(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_binary_op(I32BinaryInstr::GEU, lhs, rhs, scratch_variable_map, potential_target)
    }

    // i64 arithmetic and bitwise operations
    fn gen_i64_add(&mut self, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>, lhs: &MapperLocation, rhs: &MapperLocation) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_register_pair(self, scratch_variable_map);
        let dest_register = self.get_dest_register_pair(
            potential_target,
            scratch_variable_map,
            &vec![MapperLocation::RegisterPair(lhs_register), MapperLocation::RegisterPair(rhs_register)],
        );
        self.push_instruction(RiscvInstr::ADD { rd: dest_register.low(), rs1: lhs_register.low(), rs2: rhs_register.low() });
        self.push_instruction(RiscvInstr::SLTU { rd: dest_register.high(), rs1: dest_register.low(), rs2: lhs_register.low() });
        self.push_instruction(RiscvInstr::ADD { rd: dest_register.high(), rs1: dest_register.high(), rs2: lhs_register.high() });
        self.push_instruction(RiscvInstr::ADD { rd: dest_register.high(), rs1: dest_register.high(), rs2: rhs_register.high() });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_sub(&mut self, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>, lhs: &MapperLocation, rhs: &MapperLocation) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_register_pair(self, scratch_variable_map);
        let dest_register = self.get_dest_register_pair(
            potential_target,
            scratch_variable_map,
            &vec![MapperLocation::RegisterPair(lhs_register), MapperLocation::RegisterPair(rhs_register)],
        );
        let borrow_register = self.next_available_gp_register(
            scratch_variable_map,
            &vec![
                MapperLocation::RegisterPair(lhs_register),
                MapperLocation::RegisterPair(rhs_register),
                MapperLocation::RegisterPair(dest_register),
            ],
        );
        self.push_instruction(RiscvInstr::SLTU { rd: borrow_register, rs1: lhs_register.low(), rs2: rhs_register.low() });
        self.push_instruction(RiscvInstr::SUB { rd: dest_register.low(), rs1: lhs_register.low(), rs2: rhs_register.low() });
        self.push_instruction(RiscvInstr::SUB { rd: dest_register.high(), rs1: lhs_register.high(), rs2: rhs_register.high() });
        self.push_instruction(RiscvInstr::SUB { rd: dest_register.high(), rs1: dest_register.high(), rs2: borrow_register });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_and(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_register_pair(self, scratch_variable_map);
        let dest_register = self.get_dest_register_pair(
            potential_target,
            scratch_variable_map,
            &vec![MapperLocation::RegisterPair(lhs_register), MapperLocation::RegisterPair(rhs_register)],
        );
        self.push_instruction(RiscvInstr::AND { rd: dest_register.low(), rs1: lhs_register.low(), rs2: rhs_register.low() });
        self.push_instruction(RiscvInstr::AND { rd: dest_register.high(), rs1: lhs_register.high(), rs2: rhs_register.high() });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_or(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_register_pair(self, scratch_variable_map);
        let dest_register = self.get_dest_register_pair(
            potential_target,
            scratch_variable_map,
            &vec![MapperLocation::RegisterPair(lhs_register), MapperLocation::RegisterPair(rhs_register)],
        );
        self.push_instruction(RiscvInstr::OR { rd: dest_register.low(), rs1: lhs_register.low(), rs2: rhs_register.low() });
        self.push_instruction(RiscvInstr::OR { rd: dest_register.high(), rs1: lhs_register.high(), rs2: rhs_register.high() });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_xor(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_register_pair(self, scratch_variable_map);
        let dest_register = self.get_dest_register_pair(
            potential_target,
            scratch_variable_map,
            &vec![MapperLocation::RegisterPair(lhs_register), MapperLocation::RegisterPair(rhs_register)],
        );
        self.push_instruction(RiscvInstr::XOR { rd: dest_register.low(), rs1: lhs_register.low(), rs2: rhs_register.low() });
        self.push_instruction(RiscvInstr::XOR { rd: dest_register.high(), rs1: lhs_register.high(), rs2: rhs_register.high() });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_mul(&mut self, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>, lhs: &MapperLocation, rhs: &MapperLocation) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_register_pair(self, scratch_variable_map);
        let dest_register = self.get_dest_register_pair(
            potential_target,
            scratch_variable_map,
            &vec![MapperLocation::RegisterPair(lhs_register), MapperLocation::RegisterPair(rhs_register)],
        );
        let temp_register = self.next_available_gp_register(
            scratch_variable_map,
            &vec![
                MapperLocation::RegisterPair(lhs_register),
                MapperLocation::RegisterPair(rhs_register),
                MapperLocation::RegisterPair(dest_register),
            ],
        );

        // Compute the low 64 bits of (lhs * rhs) using 32-bit partial products.
        self.push_instruction(RiscvInstr::MUL { rd: dest_register.low(), rs1: lhs_register.low(), rs2: rhs_register.low() });
        self.push_instruction(RiscvInstr::MULHU { rd: dest_register.high(), rs1: lhs_register.low(), rs2: rhs_register.low() });
        self.push_instruction(RiscvInstr::MUL { rd: temp_register, rs1: lhs_register.low(), rs2: rhs_register.high() });
        self.push_instruction(RiscvInstr::ADD { rd: dest_register.high(), rs1: dest_register.high(), rs2: temp_register });
        self.push_instruction(RiscvInstr::MUL { rd: temp_register, rs1: lhs_register.high(), rs2: rhs_register.low() });
        self.push_instruction(RiscvInstr::ADD { rd: dest_register.high(), rs1: dest_register.high(), rs2: temp_register });

        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    // Both halves must be equal for overall equality.
    fn gen_i64_eq(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_register_pair(self, scratch_variable_map);
        let dest_register = self.get_dest_gp_register(
            potential_target,
            scratch_variable_map,
            &vec![MapperLocation::RegisterPair(lhs_register), MapperLocation::RegisterPair(rhs_register)],
        );
        let temp_register = self.next_available_gp_register(
            scratch_variable_map,
            &vec![
                MapperLocation::RegisterPair(lhs_register),
                MapperLocation::RegisterPair(rhs_register),
                MapperLocation::GPRegister(dest_register),
            ],
        );
        self.push_instruction(RiscvInstr::XOR { rd: dest_register, rs1: lhs_register.low(), rs2: rhs_register.low() });
        self.push_instruction(RiscvInstr::XOR { rd: temp_register, rs1: lhs_register.high(), rs2: rhs_register.high() });
        self.push_instruction(RiscvInstr::OR { rd: dest_register, rs1: dest_register, rs2: temp_register });
        self.push_instruction(RiscvInstr::SLTIU { rd: dest_register, rs1: dest_register, imm: Const12::new(1) });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    // Either half being different means overall inequality.
    fn gen_i64_ne(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let (lhs_register, rhs_register) = (lhs, rhs).map_to_register_pair(self, scratch_variable_map);
        let dest_register = self.get_dest_gp_register(
            potential_target,
            scratch_variable_map,
            &vec![MapperLocation::RegisterPair(lhs_register), MapperLocation::RegisterPair(rhs_register)],
        );
        let temp_register = self.next_available_gp_register(
            scratch_variable_map,
            &vec![
                MapperLocation::RegisterPair(lhs_register),
                MapperLocation::RegisterPair(rhs_register),
                MapperLocation::GPRegister(dest_register),
            ],
        );
        self.push_instruction(RiscvInstr::XOR { rd: dest_register, rs1: lhs_register.low(), rs2: rhs_register.low() });
        self.push_instruction(RiscvInstr::XOR { rd: temp_register, rs1: lhs_register.high(), rs2: rhs_register.high() });
        self.push_instruction(RiscvInstr::OR { rd: dest_register, rs1: dest_register, rs2: temp_register });
        self.push_instruction(RiscvInstr::SLTU { rd: dest_register, rs1: GPRegister(0), rs2: dest_register });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_comparison_op(
        &mut self,
        lhs: &MapperLocation,
        rhs: &MapperLocation,
        scratch_variable_map: &mut Vec<MapperLocation>,
        potential_target: Option<&MapperLocation>,
        signed_high: bool,
        swap_operands: bool,
        invert_result: bool,
    ) -> MapperLocation {
        let (left, right) = if swap_operands { (rhs, lhs) } else { (lhs, rhs) };
        let left_pair = left.map_to_register_pair(None, self, scratch_variable_map, &vec![]);
        let right_pair = right.map_to_register_pair(
            None,
            self,
            scratch_variable_map,
            &vec![MapperLocation::RegisterPair(left_pair)],
        );
        let dest_register = self.get_dest_gp_register(
            potential_target,
            scratch_variable_map,
            &vec![MapperLocation::RegisterPair(left_pair), MapperLocation::RegisterPair(right_pair)],
        );

        let mut used = vec![
            MapperLocation::RegisterPair(left_pair),
            MapperLocation::RegisterPair(right_pair),
            MapperLocation::GPRegister(dest_register),
        ];
        let hi_eq = self.next_available_gp_register(scratch_variable_map, &used);
        used.push(MapperLocation::GPRegister(hi_eq));
        let lo_lt = self.next_available_gp_register(scratch_variable_map, &used);
        used.push(MapperLocation::GPRegister(lo_lt));
        let hi_diff = self.next_available_gp_register(scratch_variable_map, &used);

        if signed_high {
            self.push_instruction(RiscvInstr::SLT { rd: dest_register, rs1: left_pair.high(), rs2: right_pair.high() });
        } else {
            self.push_instruction(RiscvInstr::SLTU { rd: dest_register, rs1: left_pair.high(), rs2: right_pair.high() });
        }
        self.push_instruction(RiscvInstr::XOR { rd: hi_diff, rs1: left_pair.high(), rs2: right_pair.high() });
        self.push_instruction(RiscvInstr::SLTIU { rd: hi_eq, rs1: hi_diff, imm: Const12::new(1) });
        self.push_instruction(RiscvInstr::SLTU { rd: lo_lt, rs1: left_pair.low(), rs2: right_pair.low() });
        self.push_instruction(RiscvInstr::AND { rd: hi_eq, rs1: hi_eq, rs2: lo_lt });
        self.push_instruction(RiscvInstr::OR { rd: dest_register, rs1: dest_register, rs2: hi_eq });

        if invert_result {
            self.push_instruction(RiscvInstr::XORI { rd: dest_register, rs1: dest_register, imm: Const12::new(1) });
        }

        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_lts(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i64_comparison_op(lhs, rhs, scratch_variable_map, potential_target, true, false, false)
    }

    fn gen_i64_ltu(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i64_comparison_op(lhs, rhs, scratch_variable_map, potential_target, false, false, false)
    }

    fn gen_i64_ges(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i64_comparison_op(lhs, rhs, scratch_variable_map, potential_target, true, false, true)
    }

    fn gen_i64_geu(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i64_comparison_op(lhs, rhs, scratch_variable_map, potential_target, false, false, true)
    }

    fn gen_i64_les(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i64_comparison_op(lhs, rhs, scratch_variable_map, potential_target, true, true, true)
    }

    fn gen_i64_leu(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i64_comparison_op(lhs, rhs, scratch_variable_map, potential_target, false, true, true)
    }

    fn gen_i64_gts(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i64_comparison_op(lhs, rhs, scratch_variable_map, potential_target, true, true, false)
    }

    fn gen_i64_gtu(&mut self, lhs: &MapperLocation, rhs: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i64_comparison_op(lhs, rhs, scratch_variable_map, potential_target, false, true, false)
    }




    // ================================================================================
    // UNARY AND CONVERSION OPERATION GENERATORS
    // ================================================================================

    fn gen_i32_unary_op(&mut self, instr: I32UnaryInstr, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_gp_register(None, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        match instr {
            I32UnaryInstr::CPOP => self.push_instruction(RiscvInstr::CPOP { rd: dest_register, rs1: child_register }),
            I32UnaryInstr::CLZ => self.push_instruction(RiscvInstr::CLZ { rd: dest_register, rs1: child_register }),
            I32UnaryInstr::CTZ => self.push_instruction(RiscvInstr::CTZ { rd: dest_register, rs1: child_register }),
            I32UnaryInstr::EQZ => self.push_instruction(RiscvInstr::SLTIU { rd: dest_register, rs1: child_register, imm: Const12::new(1)}), // x == 0 <=> x < 1 with x unsigned
        }
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i32_clz(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_unary_op(I32UnaryInstr::CLZ, child, scratch_variable_map, potential_target)
    }

    fn gen_i32_ctz(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_unary_op(I32UnaryInstr::CTZ, child, scratch_variable_map, potential_target)
    }

    // Generate simple unary operations using macro
    fn gen_i32_popcnt(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_unary_op(I32UnaryInstr::CPOP, child, scratch_variable_map, potential_target)
    }

    // i32 version: direct EQ with 0
    fn gen_i32_eqz(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_i32_unary_op(I32UnaryInstr::EQZ, child, scratch_variable_map, potential_target)
    }

    fn gen_i64_popcnt(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let RegisterPair { low: src_low, high: src_high } = child.map_to_register_pair(None, self, scratch_variable_map, &vec![]);
        let RegisterPair { low: dest_low, high: dest_high } = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::CPOP { rd: dest_low, rs1: src_low });
        self.push_instruction(RiscvInstr::CPOP { rd: dest_high, rs1: src_high });
        self.push_instruction(RiscvInstr::ADD { rd: dest_low, rs1: dest_low, rs2: dest_high });
        self.push_instruction(RiscvInstr::ADDI { rd: dest_high, rs1: GPRegister(0), imm: Const12::new(0) });
        RegisterPair { low: dest_low, high: dest_high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_clz(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let RegisterPair { low: src_low, high: src_high } = child.map_to_register_pair(None, self, scratch_variable_map, &vec![]);
        let RegisterPair { low: dest_low, high: dest_high } = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);

        // if high == 0: result = clz(low) + 32 else result = clz(high)
        self.push_instruction(RiscvInstr::BEQ { rs1: src_high, rs2: GPRegister(0), imm: Const12::new(12) });
        self.push_instruction_no_compress(RiscvInstr::CLZ { rd: dest_low, rs1: src_high });
        self.push_instruction_no_compress(RiscvInstr::JAL { rd: GPRegister(0), imm: Const20::new(12) });
        self.push_instruction_no_compress(RiscvInstr::CLZ { rd: dest_low, rs1: src_low });
        self.push_instruction_no_compress(RiscvInstr::ADDI { rd: dest_low, rs1: dest_low, imm: Const12::new(32) });
        self.push_instruction_no_compress(RiscvInstr::ADDI { rd: dest_high, rs1: GPRegister(0), imm: Const12::new(0) });

        RegisterPair { low: dest_low, high: dest_high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_ctz(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let RegisterPair { low: src_low, high: src_high } = child.map_to_register_pair(None, self, scratch_variable_map, &vec![]);
        let RegisterPair { low: dest_low, high: dest_high } = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);

        // if low == 0: result = ctz(high) + 32 else result = ctz(low)
        self.push_instruction(RiscvInstr::BEQ { rs1: src_low, rs2: GPRegister(0), imm: Const12::new(12) });
        self.push_instruction_no_compress(RiscvInstr::CTZ { rd: dest_low, rs1: src_low });
        self.push_instruction_no_compress(RiscvInstr::JAL { rd: GPRegister(0), imm: Const20::new(12) });
        self.push_instruction_no_compress(RiscvInstr::CTZ { rd: dest_low, rs1: src_high });
        self.push_instruction_no_compress(RiscvInstr::ADDI { rd: dest_low, rs1: dest_low, imm: Const12::new(32) });
        self.push_instruction_no_compress(RiscvInstr::ADDI { rd: dest_high, rs1: GPRegister(0), imm: Const12::new(0) });

        RegisterPair { low: dest_low, high: dest_high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    // i64 version: OR both halves then EQ with 0
    fn gen_i64_eqz(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let RegisterPair { low, high } = child.map_to_register_pair(None, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction( RiscvInstr::OR { rd: dest_register, rs1: low, rs2: high });
        self.push_instruction(RiscvInstr::SLTIU { rd: dest_register, rs1: dest_register, imm: Const12::new(1) });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_extend_i32u(&mut self, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>, child: &MapperLocation) -> MapperLocation {
        let child_gp = child.map_to_gp_register(None, self, scratch_variable_map, &vec![]);
        let RegisterPair { low: dest_low, high: dest_high } = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::ADDI { rd: dest_low, rs1: child_gp, imm: Const12::new(0) });
        self.push_instruction(RiscvInstr::ADDI { rd: dest_high, rs1: GPRegister(0), imm: Const12::new(0) });
        RegisterPair { low: dest_low, high: dest_high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    // Could it be optimized to reuse the lower register as a destination?
    fn gen_i64_extend_i32s(&mut self, potential_target: Option<&MapperLocation>, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let child_gp = child.map_to_gp_register(None, self, scratch_variable_map, &vec![]);
        let RegisterPair { low: dest_low, high: dest_high } = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::ADDI { rd: dest_low, rs1: child_gp, imm: Const12::new(0) });
        self.push_instruction(RiscvInstr::SRAI { rd: dest_high, rs1: dest_low, imm: ShAmt5::new(31) });
        RegisterPair { low: dest_low, high: dest_high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_f32_unary_op(&mut self, instr: FPUnaryInstr, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_fp_register(None, ValueSize::Word, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        match instr {
            FPUnaryInstr::ABS => self.push_instruction(RiscvInstr::FSGNJXS {
                rd: dest_register,
                rs1: child_register,
                rs2: child_register,
            }),
            FPUnaryInstr::NEG => self.push_instruction(RiscvInstr::FSGNJNS {
                rd: dest_register,
                rs1: child_register,
                rs2: child_register,
            }),
            FPUnaryInstr::SQRT => self.push_instruction(RiscvInstr::FSQRTS {
                rd: dest_register,
                rs1: child_register,
                rm: RoundingMode::RNE,
            }),
        }
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::Word)
    }

    fn gen_f32_abs(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_unary_op(FPUnaryInstr::ABS, child, scratch_variable_map, potential_target)
    }

    fn gen_f32_neg(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_unary_op(FPUnaryInstr::NEG, child, scratch_variable_map, potential_target)
    }

    fn gen_f32_sqrt(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f32_unary_op(FPUnaryInstr::SQRT, child, scratch_variable_map, potential_target)
    }

    fn gen_f64_unary_op(&mut self, instr: FPUnaryInstr, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_fp_register(None, ValueSize::DoubleWord, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        match instr {
            FPUnaryInstr::ABS => self.push_instruction(RiscvInstr::FSGNJXD {
                rd: dest_register,
                rs1: child_register,
                rs2: child_register,
            }),
            FPUnaryInstr::NEG => self.push_instruction(RiscvInstr::FSGNJND {
                rd: dest_register,
                rs1: child_register,
                rs2: child_register,
            }),
            FPUnaryInstr::SQRT => self.push_instruction(RiscvInstr::FSQRTD {
                rd: dest_register,
                rs1: child_register,
                rm: RoundingMode::RNE,
            }),
        }
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::DoubleWord)
    }

    fn gen_f64_abs(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_unary_op(FPUnaryInstr::ABS, child, scratch_variable_map, potential_target)
    }

    fn gen_f64_neg(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_unary_op(FPUnaryInstr::NEG, child, scratch_variable_map, potential_target)
    }

    fn gen_f64_sqrt(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        self.gen_f64_unary_op(FPUnaryInstr::SQRT, child, scratch_variable_map, potential_target)
    }
    
    fn gen_f32_convert_i32s(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_gp_register(None, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FCVTSW { rd: dest_register, rs1: child_register, rm: RoundingMode::RNE });
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::Word)
    }

    // Convert i32u to f32
    fn gen_f32_convert_i32u(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_gp_register(None, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FCVTSWU { rd: dest_register, rs1: child_register, rm: RoundingMode::RNE });
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::Word)
    }

    fn gen_i32_wrap_i64(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        match potential_target {
            None => child.lower_half(),
            Some(target) => child.lower_half().map_to_location(target, self, scratch_variable_map, &vec![])
        }
    }

    fn gen_i32_trunc_f32u(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_fp_register(None, ValueSize::Word, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FCVTWUS { rd: dest_register, rs1: child_register, rm: RoundingMode::RTZ });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i32_trunc_f32s(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_fp_register(None, ValueSize::Word, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FCVTWS { rd: dest_register, rs1: child_register, rm: RoundingMode::RTZ });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i32_trunc_f64s(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_fp_register(None, ValueSize::DoubleWord, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FCVTWD { rd: dest_register, rs1: child_register, rm: RoundingMode::RTZ });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i32_trunc_f64u(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_fp_register(None, ValueSize::DoubleWord, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FCVTWUD { rd: dest_register, rs1: child_register, rm: RoundingMode::RTZ });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_f64_convert_i32s(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_gp_register(None, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FCVTDW { rd: dest_register, rs1: child_register, rm: RoundingMode::RNE });
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::DoubleWord)
    }

    fn gen_f64_convert_i32u(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_gp_register(None, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FCVTDWU { rd: dest_register, rs1: child_register, rm: RoundingMode::RNE });
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::DoubleWord)
    }

    fn gen_f32_demote_f64(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_fp_register(None, ValueSize::DoubleWord, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FCVTSD { rd: dest_register, rs1: child_register, rm: RoundingMode::RNE });
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::Word)
    }

    fn gen_f64_promote_f32(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_fp_register(None, ValueSize::Word, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FCVTDS { rd: dest_register, rs1: child_register, rm: RoundingMode::RNE });
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::DoubleWord)
    }

    fn gen_i32_reinterpret_f32(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_fp_register(None, ValueSize::Word, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FMVXW { rd: dest_register, rs1: child_register });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_reinterpret_f64(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_fp_register(None, ValueSize::DoubleWord, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);
        let scratch_height = StackHeight::from((self.modeled_stack_height.to_usize() + 15) & !7);
        let scratch_offset = self.stack_logical_offset_to_imm(scratch_height);
        self.push_instruction(RiscvInstr::FSD { rs2: child_register, rs1: STACK_POINTER, imm: Const12::new(scratch_offset) });
        self.push_instruction(RiscvInstr::LW { rd: dest_register.low(), rs1: STACK_POINTER, imm: Const12::new(scratch_offset) });
        self.push_instruction(RiscvInstr::LW { rd: dest_register.high(), rs1: STACK_POINTER, imm: Const12::new(scratch_offset + 4) });
        dest_register.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_f32_reinterpret_i32(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_gp_register(None, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FMVWX { rd: dest_register, rs1: child_register });
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::Word)
    }

    fn gen_f64_reinterpret_i64(&mut self, child: &MapperLocation, scratch_variable_map: &mut Vec<MapperLocation>, potential_target: Option<&MapperLocation>) -> MapperLocation {
        let child_register = child.map_to_register_pair(None, self, scratch_variable_map, &vec![]);
        let dest_register = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        let scratch_height = StackHeight::from((self.modeled_stack_height.to_usize() + 15) & !7);
        let scratch_offset = self.stack_logical_offset_to_imm(scratch_height);
        self.push_instruction(RiscvInstr::SW { rs2: child_register.low(), rs1: STACK_POINTER, imm: Const12::new(scratch_offset) });
        self.push_instruction(RiscvInstr::SW { rs2: child_register.high(), rs1: STACK_POINTER, imm: Const12::new(scratch_offset + 4) });
        self.push_instruction(RiscvInstr::FLD { rd: dest_register, rs1: STACK_POINTER, imm: Const12::new(scratch_offset) });
        dest_register.map_to_location(potential_target, self, scratch_variable_map, ValueSize::DoubleWord)
    }


    // ====================================================================
    // MEMORY OPERATION GENERATORS
    // ====================================================================


    fn fits_signed_12(value: i32) -> bool {
        (-2048..=2047).contains(&value)
    }

    fn linear_memory_address(
        &mut self,
        child: &MapperLocation,
        offset: u32,
        scratch_variable_map: &mut Vec<MapperLocation>,
    ) -> GPRegister {
        let child_register = child.map_to_gp_register(None, self, scratch_variable_map, &vec![]);
        let address = self.next_available_gp_register(
            scratch_variable_map,
            &vec![MapperLocation::GPRegister(child_register)],
        );
        self.push_instruction(RiscvInstr::ADDI {
            rd: address,
            rs1: child_register,
            imm: Const12::new(0),
        });

        if offset != 0 {
            if Self::fits_signed_12(offset as i32) {
                self.push_instruction(RiscvInstr::ADDI {
                    rd: address,
                    rs1: address,
                    imm: Const12::new(offset as i16),
                });
            } else {
                let offset_register = MapperLocation::Immediate(Immediate::I32(offset)).map_to_gp_register(
                    None,
                    self,
                    scratch_variable_map,
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
        address
    }

    fn gen_i32_load(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let dest = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::LW { rd: dest, rs1: address, imm: Const12::new(0) });
        dest.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i32_load8s(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let dest = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::LB { rd: dest, rs1: address, imm: Const12::new(0) });
        dest.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i32_load8u(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let dest = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::LBU { rd: dest, rs1: address, imm: Const12::new(0) });
        dest.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i32_load16s(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let dest = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::LH { rd: dest, rs1: address, imm: Const12::new(0) });
        dest.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i32_load16u(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let dest = self.get_dest_gp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::LHU { rd: dest, rs1: address, imm: Const12::new(0) });
        dest.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_load(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let RegisterPair { low, high } = self.get_dest_register_pair(
            potential_target,
            scratch_variable_map,
            &vec![MapperLocation::GPRegister(address)],
        );
        self.push_instruction(RiscvInstr::LW { rd: low, rs1: address, imm: Const12::new(0) });
        self.push_instruction(RiscvInstr::LW { rd: high, rs1: address, imm: Const12::new(4) });
        RegisterPair { low, high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_load8s(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let RegisterPair { low, high } = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::LB { rd: low, rs1: address, imm: Const12::new(0) });
        self.push_instruction(RiscvInstr::SRAI { rd: high, rs1: low, imm: ShAmt5::new(31) });
        RegisterPair { low, high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_load8u(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let RegisterPair { low, high } = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::LBU { rd: low, rs1: address, imm: Const12::new(0) });
        self.push_instruction(RiscvInstr::ADDI { rd: high, rs1: GPRegister(0), imm: Const12::new(0) });
        RegisterPair { low, high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_load16s(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let RegisterPair { low, high } = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::LH { rd: low, rs1: address, imm: Const12::new(0) });
        self.push_instruction(RiscvInstr::SRAI { rd: high, rs1: low, imm: ShAmt5::new(31) });
        RegisterPair { low, high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_load16u(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let RegisterPair { low, high } = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::LHU { rd: low, rs1: address, imm: Const12::new(0) });
        self.push_instruction(RiscvInstr::ADDI { rd: high, rs1: GPRegister(0), imm: Const12::new(0) });
        RegisterPair { low, high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_load32s(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let RegisterPair { low, high } = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::LW { rd: low, rs1: address, imm: Const12::new(0) });
        self.push_instruction(RiscvInstr::SRAI { rd: high, rs1: low, imm: ShAmt5::new(31) });
        RegisterPair { low, high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_i64_load32u(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let RegisterPair { low, high } = self.get_dest_register_pair(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::LW { rd: low, rs1: address, imm: Const12::new(0) });
        self.push_instruction(RiscvInstr::ADDI { rd: high, rs1: GPRegister(0), imm: Const12::new(0) });
        RegisterPair { low, high }.map_to_location(potential_target, self, scratch_variable_map)
    }

    fn gen_f32_load(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let dest = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FLW { rd: dest, rs1: address, imm: Const12::new(0) });
        dest.map_to_location(potential_target, self, scratch_variable_map, ValueSize::Word)
    }

    fn gen_f64_load(&mut self, child: &MapperLocation, offset: u32, _align: u8, potential_target: Option<&MapperLocation>, scratch_variable_map: &mut Vec<MapperLocation>) -> MapperLocation {
        let address = self.linear_memory_address(child, offset, scratch_variable_map);
        let dest = self.get_dest_fp_register(potential_target, scratch_variable_map, &vec![]);
        self.push_instruction(RiscvInstr::FLD { rd: dest, rs1: address, imm: Const12::new(0) });
        dest.map_to_location(potential_target, self, scratch_variable_map, ValueSize::DoubleWord)
    }
}

// ====================================================================
// END OF VB RESOLUTION MODULE
// ====================================================================
// This completes the VBResolver implementation with comprehensive support
// for all WebAssembly operations and Aurix-specific machine code generation.