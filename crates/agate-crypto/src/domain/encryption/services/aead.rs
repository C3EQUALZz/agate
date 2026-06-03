use super::super::values::{AeadAlgo, AssociatedData, Ciphertext, Nonce};
use crate::domain::common::errors::CryptoError;

/// Authenticated-encryption strategy. The bound key lives inside the concrete
/// implementation (built by a factory from a `SecretKey`); callers supply only
/// the per-message nonce, associated data, and payload.
pub trait Aead: Send + Sync {
    fn algo(&self) -> AeadAlgo;

    fn encrypt(
        &self,
        nonce: &Nonce,
        aad: &AssociatedData,
        plaintext: &[u8],
    ) -> Result<Ciphertext, CryptoError>;

    fn decrypt(
        &self,
        nonce: &Nonce,
        aad: &AssociatedData,
        ciphertext: &Ciphertext,
    ) -> Result<Vec<u8>, CryptoError>;
}
