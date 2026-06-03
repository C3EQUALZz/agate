use crate::domain::common::values::ValueObject;

/// A nonce (number-used-once) for an AEAD operation. Length is validated
/// against the chosen algorithm when the cipher is applied, not here.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Nonce(Vec<u8>);

impl Nonce {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl ValueObject for Nonce {}
