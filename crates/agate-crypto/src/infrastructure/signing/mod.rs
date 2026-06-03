//! RustCrypto-backed signing strategies and their factory.

#[cfg(feature = "ed25519")]
pub mod ed25519;

mod rust_crypto_signature_factory;

pub use rust_crypto_signature_factory::RustCryptoSignatureFactory;
