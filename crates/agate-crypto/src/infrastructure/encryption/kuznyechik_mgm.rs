//! Kuznyechik-MGM backend: GOST R 34.12-2015 (128-bit block) in Multilinear
//! Galois Mode (cargo feature `gost-cipher`).

use std::sync::Arc;

use aead::NewAead;
use kuznyechik::Kuznyechik;
use mgm::Mgm;

use super::adapter::RustCryptoAead;
use crate::domain::common::errors::CryptoError;
use crate::domain::encryption::{Aead, AeadAlgo};

pub(super) fn build(key: &[u8]) -> Result<Arc<dyn Aead>, CryptoError> {
    let cipher = Mgm::<Kuznyechik>::new_from_slice(key).map_err(|_| {
        CryptoError::InvalidKey(format!(
            "Kuznyechik-MGM key must be {} bytes",
            AeadAlgo::KuznyechikMgm.key_len()
        ))
    })?;
    Ok(Arc::new(RustCryptoAead::new(
        cipher,
        AeadAlgo::KuznyechikMgm,
    )))
}
