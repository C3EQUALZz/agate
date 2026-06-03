//! RustCrypto-backed hashing strategies and their factory.

mod digest_hasher;
mod rust_crypto_hasher_factory;

pub use rust_crypto_hasher_factory::{CryptoRegistry, RustCryptoHasherFactory};
