//! RustCrypto-backed AEAD strategies and their factory. Each backend is
//! feature-gated; the factory returns [`CryptoError::UnsupportedAead`] for any
//! algorithm whose feature is disabled.
//!
//! [`CryptoError::UnsupportedAead`]: crate::domain::common::errors::CryptoError::UnsupportedAead

#[cfg(any(
    feature = "aes-gcm",
    feature = "chacha20poly1305",
    feature = "gost-cipher"
))]
mod adapter;

#[cfg(feature = "aes-gcm")]
mod aes_gcm;
#[cfg(feature = "chacha20poly1305")]
mod chacha20poly1305;
#[cfg(feature = "gost-cipher")]
mod kuznyechik_mgm;
#[cfg(feature = "gost-cipher")]
mod magma_mgm;

mod rust_crypto_aead_factory;

pub use rust_crypto_aead_factory::RustCryptoAeadFactory;
