use crate::domain::common::values::ValueObject;

/// 0-based position of a record (leaf) in the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LeafIndex(pub u64);

impl LeafIndex {
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ValueObject for LeafIndex {}
