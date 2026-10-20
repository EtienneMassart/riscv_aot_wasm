#![forbid(unsafe_code)]
use alloc::{boxed::Box,vec::Vec};
use defmt::Format;
use crate::{isa_model::{MapperLocation, ValueType}, translator::StackHeight};
pub(crate) type Address = u32;

#[derive(Debug, PartialEq, Clone, Format)]
pub(crate) enum AtomicVB{
    I32Const {imm: i32},
    I64Const {imm: i64},
    F32Const {imm: u32},
    F64Const {imm: u64},
    Local {index: u32},
    Global {index: u32},
    Resolved {val_type: ValueType, offset: StackHeight},
    MemorySize,
}

#[derive(Debug, PartialEq, Clone, Format)]
pub(crate) enum UnaryVB{
    I32Load {offset:Address, align:u8},
    I64Load {offset:Address, align:u8},
    F32Load {offset:Address, align:u8},
    F64Load {offset:Address, align:u8},
    I32Load8s {offset:Address, align:u8},
    I32Load8u {offset:Address, align:u8},
    I32Load16s {offset:Address, align:u8},
    I32Load16u {offset:Address, align:u8},
    I64Load8s {offset:Address, align:u8},
    I64Load8u {offset:Address, align:u8},
    I64Load16s {offset:Address, align:u8},
    I64Load16u {offset:Address, align:u8},
    I64Load32s {offset:Address, align:u8},
    I64Load32u {offset:Address, align:u8},
    I32EqZ,
    I64EqZ,
    I32Clz,
    I32Ctz,
    I32PopCnt,
    I64Clz,
    I64Ctz,
    I64PopCnt,
    F32Abs,
    F32Neg,
    F32Sqrt,
    F32Ceil,
    F32Floor,
    F32Trunc,
    F32Nearest,
    F64Abs,
    F64Neg,
    F64Sqrt,
    F64Ceil,
    F64Floor,
    F64Trunc,
    F64Nearest,
    I32WrapI64,
    I32TruncF32S,
    I32TruncF32U,
    I32TruncF64S,
    I32TruncF64U,
    I64ExtendI32S,
    I64ExtendI32U,
    I64TruncF32S,
    I64TruncF32U,
    I64TruncF64S,
    I64TruncF64U,
    F32ConvertI32S,
    F32ConvertI32U,
    F32ConvertI64S,
    F32ConvertI64U,
    F32DemoteF64,
    F64ConvertI32S,
    F64ConvertI32U,
    F64ConvertI64S,
    F64ConvertI64U,
    F64PromoteF32,
    I32ReinterpretF32,
    I64ReinterpretF64,
    F32ReinterpretI32,
    F64ReinterpretI64,
}


#[derive(Debug, PartialEq, Clone, Format)]
pub(crate) enum BinaryVB{
    I32Eq,
    I32Ne,
    I32LtS,
    I32LtU,
    I32GtS,
    I32GtU,
    I32GeS,
    I32GeU,
    I32LeS,
    I32LeU,
    I64Eq,
    I64Ne,
    I64LtS,
    I64LtU,
    I64GtS,
    I64GtU,
    I64GeS,
    I64GeU,
    I64LeS,
    I64LeU,
    F32Eq,
    F32Ne,
    F32Lt,
    F32Ge,
    F32Gt,
    F32Le,
    F64Eq,
    F64Ne,
    F64Lt,
    F64Ge,
    F64Gt,
    F64Le,
    I32Add,
    I32Sub,
    I32Mul,
    I32DivS,
    I32DivU,
    I32RemS,
    I32RemU,
    I32And,
    I32Or,
    I32Xor,
    I32Shl,
    I32ShrS,
    I32ShrU,
    I32Rotl,
    I32Rotr,
    I64Add,
    I64Sub,
    I64Mul,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    I64And,
    I64Or,
    I64Xor,
    I64Shl,
    I64ShrS,
    I64ShrU,
    I64Rotl,
    I64Rotr,
    F32Add,
    F32Sub,
    F32Mul,
    F32Div,
    F32Min,
    F32Max,
    F32CopySign,
    F64Add,
    F64Sub,
    F64Mul,
    F64Div,
    F64Min,
    F64Max,
    F64CopySign
}
#[derive(Debug, PartialEq, Clone)]
pub(crate) enum VB{
    AtomicVB(AtomicVB),
    UnaryVB {vb:UnaryVB, child: Box<VB> },
    BinaryVB{vb: BinaryVB, lhs: Box<VB>, rhs:Box<VB>},
    Select{selector:Box<VB>, lhs:Box<VB>, rhs:Box<VB>, val_type:ValueType},
}

/// ContextVB is used to keep track of the position of a VB in a tree using Zippers
#[derive(Debug, PartialEq, Clone)]
enum ContextVB{
    Top,
    ContextUnaryVB{vb:UnaryVB, context: Box<ContextVB>},
    ContextBinaryVBLhs{vb:BinaryVB, rhs:Box<VB>, context: Box<ContextVB>},
    ContextBinaryVBRhs{vb:BinaryVB, lhs:Box<VB>, context: Box<ContextVB>},
    ContextSelectSelector{lhs:Box<VB>, rhs:Box<VB>, val_type:ValueType, context: Box<ContextVB>},
    ContextSelectLhs{selector:Box<VB>, rhs:Box<VB>, val_type:ValueType, context: Box<ContextVB>},
    ContextSelectRhs{selector:Box<VB>, lhs:Box<VB>, val_type:ValueType, context: Box<ContextVB>}
}

/// VBTreeLocation is used to keep track of the position of a VB in a tree using Zippers
#[derive(Debug, PartialEq, Clone)]
struct VBTreeLocation (VB, ContextVB);

impl VBTreeLocation{
    
    /// Move the cursor to the parent of the current node
    fn move_up(self) -> Option<Self>{
        match self {
            VBTreeLocation(_ , ContextVB::Top) => None,
            VBTreeLocation(_, ContextVB::ContextUnaryVB{vb:unary_vb, context}) 
                => Some(VBTreeLocation(VB::UnaryVB {vb: unary_vb, child: Box::new(self.0)}, *context)),
            VBTreeLocation(_, ContextVB::ContextBinaryVBLhs{vb: binary_vb, rhs, context})
            => Some(VBTreeLocation(VB::BinaryVB {vb: binary_vb, lhs: Box::new(self.0), rhs}, *context)),
            VBTreeLocation(_, ContextVB::ContextBinaryVBRhs{vb: binary_vb, lhs, context})
            => Some(VBTreeLocation(VB::BinaryVB {vb: binary_vb, lhs, rhs: Box::new(self.0)}, *context)),
            VBTreeLocation(_, ContextVB::ContextSelectSelector{lhs, rhs, val_type, context})
            => Some(VBTreeLocation(VB::Select {selector: Box::new(self.0), lhs, rhs, val_type}, *context)),
            VBTreeLocation(_, ContextVB::ContextSelectLhs{selector, rhs, val_type, context})
            => Some(VBTreeLocation(VB::Select {selector, lhs: Box::new(self.0), rhs, val_type}, *context)),
            VBTreeLocation(_, ContextVB::ContextSelectRhs{selector, lhs, val_type, context})
            => Some(VBTreeLocation(VB::Select {selector, lhs, rhs: Box::new(self.0), val_type}, *context)),
        }
    }
    
    /// Move the cursor to the first child of the current node
    fn move_first_child(self) -> Option<Self>{
        match self {
            VBTreeLocation(VB::UnaryVB {vb, child}, context) => Some(VBTreeLocation(*child, ContextVB::ContextUnaryVB{vb, context: Box::new(context)})),
            VBTreeLocation(VB::BinaryVB {vb, lhs, rhs}, context) => Some(VBTreeLocation(*lhs, ContextVB::ContextBinaryVBLhs{vb, rhs, context: Box::new(context)})),
            VBTreeLocation(VB::Select {selector, lhs, rhs, val_type}, context) => Some(VBTreeLocation(*lhs, ContextVB::ContextSelectLhs{selector, rhs, val_type, context: Box::new(context)})),
            _ => None
        }
    }
    
    /// Move the cursor to the last child of the current node
    fn move_first_leaf(self) -> Self{
        let mut current = self;
        while current.0.has_child(){
            current = current.move_first_child().unwrap();
        }
        current
    }
    
    /// Move the cursor to the next sibling of the current node
    fn next_sibling(self) -> Option<Self>{
        match self {
            VBTreeLocation(_, ContextVB::Top) => None,
            VBTreeLocation(_, ContextVB::ContextUnaryVB{..}) 
                => None,
            VBTreeLocation(_, ContextVB::ContextBinaryVBLhs{vb: binary_vb, rhs, context})
            => Some(VBTreeLocation(*rhs, ContextVB::ContextBinaryVBRhs{vb: binary_vb, lhs: Box::new(self.0), context})),
            VBTreeLocation(_, ContextVB::ContextBinaryVBRhs{..})
            => None,
            VBTreeLocation(_, ContextVB::ContextSelectLhs{selector, rhs, val_type, context})
            => Some(VBTreeLocation(*rhs, ContextVB::ContextSelectRhs{selector, lhs: Box::new(self.0), val_type, context: Box::new(*context)})),
            VBTreeLocation(_, ContextVB::ContextSelectRhs{selector, lhs, val_type, context})
            => Some(VBTreeLocation(*selector, ContextVB::ContextSelectSelector{lhs, rhs: Box::new(self.0), val_type, context: Box::new(*context)})),
            VBTreeLocation(_, ContextVB::ContextSelectSelector{..})
            => None,
        }
    }
    
    /// Returns true if the current node has a next sibling
    fn has_next_sibling(&self) -> bool{
        match self {
            VBTreeLocation(_, ContextVB::Top) => false,
            VBTreeLocation(_, ContextVB::ContextUnaryVB{..}) => false,
            VBTreeLocation(_, ContextVB::ContextBinaryVBLhs{..}) => true,
            VBTreeLocation(_, ContextVB::ContextBinaryVBRhs{..}) => false,
            VBTreeLocation(_, ContextVB::ContextSelectSelector{..}) => false,
            VBTreeLocation(_, ContextVB::ContextSelectLhs{..}) => true,
            VBTreeLocation(_, ContextVB::ContextSelectRhs{..}) => true,
        }
    }
    
}

impl VB{
    
    /// Returns the size of the value represented by the VB
    ///
    /// # Arguments
    /// * `locals_map` - the local variables map used during translation to represent the physical location of the local variables
    /// * `global_variable_map` - the global variables map used during translation to represent the physical location of the global variables
    /// 
    /// # Returns
    /// The type of the value represented by the VB
    pub(crate) fn val_type (&self, locals_map: &Vec<MapperLocation>, global_variable_map:&Vec<(u32, ValueType)>) -> ValueType{
        match self {
            VB::AtomicVB(AtomicVB::I32Const{..}) => ValueType::I32,
            VB::AtomicVB(AtomicVB::I64Const{..}) => ValueType::I64,
            VB::AtomicVB(AtomicVB::F32Const{..}) => ValueType::F32,
            VB::AtomicVB(AtomicVB::F64Const{..}) => ValueType::F64,
            VB::AtomicVB(AtomicVB::Local{ index}) => locals_map[*index as usize].get_type(),
            VB::AtomicVB(AtomicVB::Global{ index}) => global_variable_map[*index as usize].1,
            VB::AtomicVB(AtomicVB::Resolved{val_type, ..}) => *val_type,
            VB::AtomicVB(AtomicVB::MemorySize) => ValueType::I32,
            VB::UnaryVB{vb, ..} => match vb {
                    UnaryVB::I32EqZ => ValueType::I32,
                    UnaryVB::I64EqZ => ValueType::I64,
                    UnaryVB::I32Clz => ValueType::I32,
                    UnaryVB::I32Ctz => ValueType::I32,
                    UnaryVB::I32PopCnt => ValueType::I32,
                    UnaryVB::I64Clz => ValueType::I64,
                    UnaryVB::I64Ctz => ValueType::I64,
                    UnaryVB::I64PopCnt => ValueType::I64,
                    UnaryVB::F32Abs => ValueType::F32,
                    UnaryVB::F32Neg => ValueType::F32,
                    UnaryVB::F32Sqrt => ValueType::F32,
                    UnaryVB::F32Ceil => ValueType::F32,
                    UnaryVB::F32Floor => ValueType::F32,
                    UnaryVB::F32Trunc => ValueType::F32,
                    UnaryVB::F32Nearest => ValueType::F32,
                    UnaryVB::F64Abs => ValueType::F64,
                    UnaryVB::F64Neg => ValueType::F64,
                    UnaryVB::F64Sqrt => ValueType::F64,
                    UnaryVB::F64Ceil => ValueType::F64,
                    UnaryVB::F64Floor => ValueType::F64,
                    UnaryVB::F64Trunc => ValueType::F64,
                    UnaryVB::F64Nearest => ValueType::F64,
                    UnaryVB::I32WrapI64 => ValueType::I32,
                    UnaryVB::I32TruncF32S => ValueType::I32,
                    UnaryVB::I32TruncF32U => ValueType::I32,
                    UnaryVB::I32TruncF64S => ValueType::I32,
                    UnaryVB::I32TruncF64U => ValueType::I32,
                    UnaryVB::I64ExtendI32S => ValueType::I64,
                    UnaryVB::I64ExtendI32U => ValueType::I64,
                    UnaryVB::I64TruncF32S => ValueType::I64,
                    UnaryVB::I64TruncF32U => ValueType::I64,
                    UnaryVB::I64TruncF64S => ValueType::I64,
                    UnaryVB::I64TruncF64U => ValueType::I64,
                    UnaryVB::F32ConvertI32S => ValueType::F32,
                    UnaryVB::F32ConvertI32U => ValueType::F32,
                    UnaryVB::F32ConvertI64S => ValueType::F32,
                    UnaryVB::F32ConvertI64U => ValueType::F32,
                    UnaryVB::F32DemoteF64 => ValueType::F32,
                    UnaryVB::F64ConvertI32S => ValueType::F64,
                    UnaryVB::F64ConvertI32U => ValueType::F64,
                    UnaryVB::F64ConvertI64S => ValueType::F64,
                    UnaryVB::F64ConvertI64U => ValueType::F64,
                    UnaryVB::F64PromoteF32 => ValueType::F64,
                    UnaryVB::I32Load { ..} => ValueType::I32,
                    UnaryVB::I64Load { ..} => ValueType::I64,
                    UnaryVB::F32Load { ..} => ValueType::F32,
                    UnaryVB::F64Load { ..} => ValueType::F64,
                    UnaryVB::I32Load8s { ..} => ValueType::I32,
                    UnaryVB::I32Load8u { ..} => ValueType::I32,
                    UnaryVB::I32Load16s { ..} => ValueType::I32,
                    UnaryVB::I32Load16u { ..} => ValueType::I32,
                    UnaryVB::I64Load8s { ..} => ValueType::I64,
                    UnaryVB::I64Load8u { ..} => ValueType::I64,
                    UnaryVB::I64Load16s { ..} => ValueType::I64,
                    UnaryVB::I64Load16u { ..} => ValueType::I64,
                    UnaryVB::I64Load32s { ..} => ValueType::I64,
                    UnaryVB::I64Load32u { ..} => ValueType::I64,
                    UnaryVB::F32ReinterpretI32 {..} => ValueType::F32,
                    UnaryVB::F64ReinterpretI64 {..} => ValueType::F64,
                    UnaryVB::I32ReinterpretF32 {..} => ValueType::I32,
                    UnaryVB::I64ReinterpretF64 {..} => ValueType::I64,
                },
                VB::BinaryVB {vb, ..} => match vb {
                    BinaryVB::I32Eq => ValueType::I32,
                    BinaryVB::I32Ne => ValueType::I32,
                    BinaryVB::I32LtS => ValueType::I32,
                    BinaryVB::I32LtU => ValueType::I32,
                    BinaryVB::I32GtS => ValueType::I32,
                    BinaryVB::I32GtU => ValueType::I32,
                    BinaryVB::I32GeS => ValueType::I32,
                    BinaryVB::I32GeU => ValueType::I32,
                    BinaryVB::I32LeS => ValueType::I32,
                    BinaryVB::I32LeU => ValueType::I32,
                    BinaryVB::I64Eq => ValueType::I64,
                    BinaryVB::I64Ne => ValueType::I64,
                    BinaryVB::I64LtS => ValueType::I64,
                    BinaryVB::I64LtU => ValueType::I64,
                    BinaryVB::I64GtS => ValueType::I64,
                    BinaryVB::I64GtU => ValueType::I64,
                    BinaryVB::I64GeS => ValueType::I64,
                    BinaryVB::I64GeU => ValueType::I64,
                    BinaryVB::I64LeS => ValueType::I64,
                    BinaryVB::I64LeU => ValueType::I64,
                    BinaryVB::F32Eq => ValueType::F32,
                    BinaryVB::F32Ne => ValueType::F32,
                    BinaryVB::F32Lt => ValueType::F32,
                    BinaryVB::F32Ge => ValueType::F32,
                    BinaryVB::F32Gt => ValueType::F32,
                    BinaryVB::F32Le => ValueType::F32,
                    BinaryVB::F64Eq => ValueType::F64,
                    BinaryVB::F64Ne => ValueType::F64,
                    BinaryVB::F64Lt => ValueType::F64,
                    BinaryVB::F64Ge => ValueType::F64,
                    BinaryVB::F64Gt => ValueType::F64,
                    BinaryVB::F64Le => ValueType::F64,
                    BinaryVB::I32Add => ValueType::I32,
                    BinaryVB::I32Sub => ValueType::I32,
                    BinaryVB::I32Mul => ValueType::I32,
                    BinaryVB::I32DivS => ValueType::I32,
                    BinaryVB::I32DivU => ValueType::I32,
                    BinaryVB::I32RemS => ValueType::I32,
                    BinaryVB::I32RemU => ValueType::I32,
                    BinaryVB::I32And => ValueType::I32,
                    BinaryVB::I32Or => ValueType::I32,
                    BinaryVB::I32Xor => ValueType::I32,
                    BinaryVB::I32Shl => ValueType::I32,
                    BinaryVB::I32ShrS => ValueType::I32,
                    BinaryVB::I32ShrU => ValueType::I32,
                    BinaryVB::I32Rotl => ValueType::I32,
                    BinaryVB::I32Rotr => ValueType::I32,
                    BinaryVB::I64Add => ValueType::I64,
                    BinaryVB::I64Sub => ValueType::I64,
                    BinaryVB::I64Mul => ValueType::I64,
                    BinaryVB::I64DivS => ValueType::I64,
                    BinaryVB::I64DivU => ValueType::I64,
                    BinaryVB::I64RemS => ValueType::I64,
                    BinaryVB::I64RemU => ValueType::I64,
                    BinaryVB::I64And => ValueType::I64,
                    BinaryVB::I64Or => ValueType::I64,
                    BinaryVB::I64Xor => ValueType::I64,
                    BinaryVB::I64Shl => ValueType::I64,
                    BinaryVB::I64ShrS => ValueType::I64,
                    BinaryVB::I64ShrU => ValueType::I64,
                    BinaryVB::I64Rotl => ValueType::I64,
                    BinaryVB::I64Rotr => ValueType::I64,
                    BinaryVB::F32Add => ValueType::F32,
                    BinaryVB::F32Sub => ValueType::F32,
                    BinaryVB::F32Mul => ValueType::F32,
                    BinaryVB::F32Div => ValueType::F32,
                    BinaryVB::F32Min => ValueType::F32,
                    BinaryVB::F32Max => ValueType::F32,
                    BinaryVB::F32CopySign => ValueType::F32,
                    BinaryVB::F64Add => ValueType::F64,
                    BinaryVB::F64Sub => ValueType::F64,
                    BinaryVB::F64Mul => ValueType::F64,
                    BinaryVB::F64Div => ValueType::F64,
                    BinaryVB::F64Min => ValueType::F64,
                    BinaryVB::F64Max => ValueType::F64,
                    BinaryVB::F64CopySign => ValueType::F64,
                },
                VB::Select{val_type, ..} => *val_type,
            }
    }
    
    /// Returns true if the VB depends on a local variable
    /// 
    /// # Arguments
    /// * `local_index` - the index of the local variable
    pub(crate) fn depends_on_local(&self, local_index: u32) -> bool{
        match self {
            VB::AtomicVB(AtomicVB::Local{index}) => *index == local_index,
            VB::AtomicVB(..) => false,
            VB::UnaryVB{child, ..} => child.depends_on_local(local_index),
            VB::BinaryVB{lhs, rhs, ..} => lhs.depends_on_local(local_index) || rhs.depends_on_local(local_index),
            VB::Select{selector, lhs, rhs, ..} => selector.depends_on_local(local_index) || lhs.depends_on_local(local_index) || rhs.depends_on_local(local_index),
        }
    }
    
    /// Returns true if the VB depends on a memory location
    
    pub(crate) fn depends_on_memory(&self) -> bool{
        match self {
            VB::AtomicVB(..) => false,
            VB::UnaryVB{child, vb} => match vb {
                UnaryVB::I32Load {..} | UnaryVB::I64Load {..} | UnaryVB::F32Load {..} | UnaryVB::F64Load {..} | UnaryVB::I32Load8s {..} | UnaryVB::I32Load8u {..} | UnaryVB::I32Load16s {..} | UnaryVB::I32Load16u {..} | UnaryVB::I64Load8s {..} | UnaryVB::I64Load8u {..} | UnaryVB::I64Load16s {..} | UnaryVB::I64Load16u {..} | UnaryVB::I64Load32s {..} | UnaryVB::I64Load32u {..} => true,
                _ => child.depends_on_memory()
            }
            VB::BinaryVB{lhs, rhs, ..} => lhs.depends_on_memory() || rhs.depends_on_memory(),
            VB::Select{selector, lhs, rhs, ..} => selector.depends_on_memory() || lhs.depends_on_memory() || rhs.depends_on_memory(),
        }
    }

    /// Performs a post-order depth-first search on the VB tree. This is used to resolve the VB tree into a sequence of instructions.
    pub(crate) fn post_order_dfs(self, mut f: impl FnMut(&VB, bool) ){
        let mut tree_location = VBTreeLocation(self, ContextVB::Top).move_first_leaf();
        loop {
            f(&tree_location.0, tree_location.1 == ContextVB::Top);
            if tree_location.has_next_sibling(){
                tree_location = tree_location.next_sibling().unwrap().move_first_leaf();
            } else {
                    if let Some(parent) = tree_location.move_up() {
                        tree_location = parent;
                    } else {
                        return;
                    }
            }
        }
       
    }

    /// Returns true if the VB has a child
    pub(crate) fn has_child(&self) -> bool {
        match self {
            VB::AtomicVB(..) => false,
            _ => true
        }
    }
}
