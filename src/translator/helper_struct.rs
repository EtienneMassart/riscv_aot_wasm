use defmt::Format;
use crate::isa_model::ValueType;


/// Wrapper for label indices used in control flow constructs. Values are unique and monotonically increasing.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Format, PartialEq)]
pub(crate) struct LabelIndex (usize);


impl From<usize> for LabelIndex {
    fn from(value: usize) -> Self {
        LabelIndex(value)
    }
}

impl LabelIndex {

    pub(crate) fn new(value: usize) -> Self {
        LabelIndex(value)
    }

    pub(crate) fn to_usize(self) -> usize {
        self.0
    }
}

/// Represents a control flow label, which can be a block, a loop or an if-else-end structure. Each label has a unique identifier.
/// For Block and Loop, there is a single LabelIndex representing the end of the block or the beginning of the loop.
/// For If, there are two LabelIndexs: one for the else branch and one for the end branch.
pub(crate) enum BlockLabel {
    BlockLoop(LabelIndex),
    If { else_label: LabelIndex, end_label: LabelIndex },
}

/// A pending control-flow fixup recorded during translation.
///
/// The jump/branch instruction is emitted with a placeholder immediate and patched in
/// a second pass once all label addresses in `cfg_label_map` are known.
pub(crate) struct CfgJob {
    pub instruction_hw_index: usize,
    pub target_label: LabelIndex,
}

/// Wrapper for stack height values used in control flow constructs. Values are in bytes.
#[repr(transparent)]
#[derive(Debug, Format, Copy, PartialEq, Clone)]
pub(crate) struct StackHeight(usize);

impl From<usize> for StackHeight {
    fn from(value: usize) -> Self {
        StackHeight(value)
    }
}

impl StackHeight {
    pub(crate) fn add(self, rhs: usize) -> Self {
        StackHeight(self.0 + rhs)
    }

    pub(crate) fn to_usize(self) -> usize {
        self.0
    }

    pub(crate) fn to_i32(self) -> i32 {
        self.0 as i32
    }

    pub(crate) fn sub(self, rhs: usize) -> Self {
        StackHeight(self.0.saturating_sub(rhs))
    }
}

/// Represents the state of the wasm stack after exiting a control flow block.
/// The first element is the height of the stack in bytes after exiting the block.
/// The second element is the type of the result value of the block, if any.
/// The stack state after exiting the block is the same as before entering the block plus the result value at the top, if any
#[derive(Debug, Clone, Copy)]
pub(crate) struct StackState {
    pub height: StackHeight,
    pub result_type: Option<ValueType>,
}

/// Represents the result of a control flow block.
/// Includes the stack state when reaching the end of the block naturally and when branching to the block's label position (e.g. with a break instruction).
/// The two states are usually the same, except for loops where the label state has no result value.
/// That is because breaking to a loop label means jumping to the beginning of the loop, not the end unlike with blocks and ifs.
pub(crate) struct BlockResult {
    pub end_state: StackState, // Stack state when reaching the end of the block naturally
    pub label_state: StackState, // Stack state when branching to this block's label position, only different for loops
}