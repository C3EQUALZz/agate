use crate::domain::common::values::SecretKey;
use crate::domain::encryption::{AeadAlgo, AssociatedData, Nonce};

/// Encrypt `plaintext` under an AEAD algorithm. `aad` is authenticated but not
/// encrypted; pass [`AssociatedData::empty`] when unused.
pub struct Encrypt {
    pub algo: AeadAlgo,
    pub key: SecretKey,
    pub nonce: Nonce,
    pub aad: AssociatedData,
    pub plaintext: Vec<u8>,
}
