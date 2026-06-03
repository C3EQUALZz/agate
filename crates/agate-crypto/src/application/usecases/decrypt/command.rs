use crate::domain::common::values::SecretKey;
use crate::domain::encryption::{AssociatedData, Ciphertext, Nonce};

/// Decrypt and authenticate `ciphertext`. The algorithm is taken from the
/// self-describing ciphertext; `aad` must match what was supplied at encryption.
pub struct Decrypt {
    pub key: SecretKey,
    pub nonce: Nonce,
    pub aad: AssociatedData,
    pub ciphertext: Ciphertext,
}
