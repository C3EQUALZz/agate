use crate::domain::common::values::ValueObject;

/// Additional authenticated data: bound into the tag but left in the clear.
/// Empty is valid and the common default.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct AssociatedData(Vec<u8>);

impl AssociatedData {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl ValueObject for AssociatedData {}
