use std::sync::Arc;

use crate::application::common::ports::AeadFactory;
use crate::domain::common::errors::CryptoError;
use crate::domain::common::values::SecretKey;
use crate::domain::encryption::{Aead, AeadAlgo};

/// Concrete [`AeadFactory`] backed by RustCrypto AEAD constructions.
pub struct RustCryptoAeadFactory;

impl AeadFactory for RustCryptoAeadFactory {
    #[cfg_attr(
        not(any(
            feature = "aes-gcm",
            feature = "chacha20poly1305",
            feature = "gost-cipher"
        )),
        allow(unused_variables)
    )]
    fn aead(&self, algo: AeadAlgo, key: &SecretKey) -> Result<Arc<dyn Aead>, CryptoError> {
        match algo {
            #[cfg(feature = "aes-gcm")]
            AeadAlgo::Aes256Gcm => super::aes_gcm::build(key.expose()),
            #[cfg(feature = "chacha20poly1305")]
            AeadAlgo::ChaCha20Poly1305 => super::chacha20poly1305::build(key.expose()),
            #[cfg(feature = "gost-cipher")]
            AeadAlgo::KuznyechikMgm => super::kuznyechik_mgm::build(key.expose()),
            #[cfg(feature = "gost-cipher")]
            AeadAlgo::MagmaMgm => super::magma_mgm::build(key.expose()),
            #[allow(unreachable_patterns)]
            _ => Err(CryptoError::UnsupportedAead(algo)),
        }
    }
}
