//! AES-256-GCM backend (cargo feature `aes-gcm`).

use std::sync::Arc;

use aead::NewAead;
use aes_gcm::Aes256Gcm;

use super::adapter::RustCryptoAead;
use crate::domain::common::errors::CryptoError;
use crate::domain::encryption::{Aead, AeadAlgo};

pub(super) fn build(key: &[u8]) -> Result<Arc<dyn Aead>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| {
        CryptoError::InvalidKey(format!(
            "AES-256-GCM key must be {} bytes",
            AeadAlgo::Aes256Gcm.key_len()
        ))
    })?;
    Ok(Arc::new(RustCryptoAead::new(cipher, AeadAlgo::Aes256Gcm)))
}
