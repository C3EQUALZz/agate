use super::AeadAlgo;
use crate::domain::common::values::ValueObject;

/// Ciphertext tagged with the algorithm that produced it. `bytes` is the AEAD
/// output: the encrypted payload with its authentication tag appended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ciphertext {
    pub algo: AeadAlgo,
    pub bytes: Vec<u8>,
}

impl Ciphertext {
    pub fn new(algo: AeadAlgo, bytes: Vec<u8>) -> Self {
        Self { algo, bytes }
    }
}

impl ValueObject for Ciphertext {}
