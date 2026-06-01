use crate::domain::common::values::ValueObject;

/// Number of leaves committed to the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeSize(pub u64);

impl TreeSize {
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ValueObject for TreeSize {}
