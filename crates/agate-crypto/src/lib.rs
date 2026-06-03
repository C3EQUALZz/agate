//! # agate-crypto
//!
//! Crypto agility for Agate: pluggable, self-describing **hash**, **signature**
//! and **AEAD** algorithms (incl. GOST — Streebog, Kuznyechik, Magma).
//!
//! Architectural note: this crate is a *generic subdomain* published as a
//! library — **not** a DDD shared kernel. Depending on it is like depending on
//! `ring` or `sha2`: a stable technical capability, not a shared domain model.
//! Internally it still follows Clean Architecture so the design stays uniform
//! with the bounded-context crates:
//!
//! - [`domain`] — pure, dependency-free: self-describing algorithm values and
//!   the strategy traits ([`Hasher`], [`Signer`], [`Verifier`], [`Aead`]).
//! - [`application`] — abstract-factory ports and thin use cases over them.
//! - [`infrastructure`] — concrete RustCrypto backends and the factories.
//!
//! The two patterns at the core are **strategy** (the algorithm traits) and
//! **abstract factory** (resolve a self-describing algorithm to a strategy).

pub mod application;
pub mod domain;
pub mod infrastructure;

// --- Stable public surface (so consumers can write `agate_crypto::Digest`). ---

pub use domain::common::{CryptoError, SecretKey};
pub use domain::encryption::{Aead, AeadAlgo, AssociatedData, Ciphertext, Nonce};
pub use domain::hashing::{Digest, HashAlgo, Hasher};
pub use domain::signing::{KeyId, SignAlgo, Signature, Signer, Verifier};

pub use application::common::ports::{AeadFactory, HasherFactory, SignatureFactory};

pub use infrastructure::encryption::RustCryptoAeadFactory;
pub use infrastructure::hashing::{CryptoRegistry, RustCryptoHasherFactory};
pub use infrastructure::signing::RustCryptoSignatureFactory;

/// Backward-compatible module path: `agate_crypto::ed25519::Ed25519Signer`.
#[cfg(feature = "ed25519")]
pub mod ed25519 {
    pub use crate::infrastructure::signing::ed25519::{Ed25519Signer, Ed25519Verifier};
}
