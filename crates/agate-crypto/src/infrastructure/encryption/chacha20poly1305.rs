//! ChaCha20-Poly1305 backend (cargo feature `chacha20poly1305`).

use std::sync::Arc;

use aead::NewAead;
use chacha20poly1305::ChaCha20Poly1305;

use super::adapter::RustCryptoAead;
use crate::domain::common::errors::CryptoError;
use crate::domain::encryption::{Aead, AeadAlgo};

pub(super) fn build(key: &[u8]) -> Result<Arc<dyn Aead>, CryptoError> {
    let cipher = ChaCha20Poly1305::new_from_slice(key).map_err(|_| {
        CryptoError::InvalidKey(format!(
            "ChaCha20-Poly1305 key must be {} bytes",
            AeadAlgo::ChaCha20Poly1305.key_len()
        ))
    })?;
    Ok(Arc::new(RustCryptoAead::new(
        cipher,
        AeadAlgo::ChaCha20Poly1305,
    )))
}
