//! # agate-crypto
//!
//! Crypto agility for Agate: pluggable, self-describing hash and signature
//! algorithms selectable by the user (incl. GOST/Streebog).
//!
//! Architectural note: this crate is a *generic subdomain* published as a
//! library — **not** a DDD shared kernel. Depending on it is like depending on
//! `ring` or `sha2`: a stable technical capability, not a shared domain model.
//!
//! The core (algorithm enums, [`Digest`], [`Signature`], and the [`Hasher`] /
//! [`Signer`] / [`Verifier`] traits) carries no third-party crypto dependency.
//! Concrete implementations live behind cargo features so heavier or
//! less-mature backends (e.g. GOST) stay opt-in and isolated.

use std::fmt::{self, Write as _};
use std::sync::Arc;

mod registry;

#[cfg(feature = "ed25519")]
pub mod ed25519;

pub use registry::CryptoRegistry;

/// Self-describing hash algorithm identifier (think JWS `alg` / multicodec).
///
/// The identifier is persisted alongside every [`Digest`] so that records
/// remain verifiable even after the configured default algorithm changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HashAlgo {
    Sha256,
    Sha512,
    Sha3_256,
    /// GOST R 34.11-2012, 256-bit (Streebog).
    Streebog256,
    /// GOST R 34.11-2012, 512-bit (Streebog).
    Streebog512,
}

impl HashAlgo {
    /// Stable byte code for self-describing serialization / canonical forms.
    pub fn code(self) -> u8 {
        match self {
            HashAlgo::Sha256 => 1,
            HashAlgo::Sha512 => 2,
            HashAlgo::Sha3_256 => 3,
            HashAlgo::Streebog256 => 4,
            HashAlgo::Streebog512 => 5,
        }
    }
}

/// Self-describing signature algorithm identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignAlgo {
    Ed25519,
    /// GOST R 34.10-2012, 256-bit (not yet implemented).
    GostR3410_2012_256,
    /// GOST R 34.10-2012, 512-bit (not yet implemented).
    GostR3410_2012_512,
}

/// A hash value tagged with the algorithm that produced it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Digest {
    pub algo: HashAlgo,
    pub bytes: Vec<u8>,
}

impl Digest {
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(self.bytes.len() * 2);
        for b in &self.bytes {
            let _ = write!(s, "{b:02x}");
        }
        s
    }
}

/// Identifier of the key that produced a [`Signature`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyId(pub String);

/// A signature tagged with its algorithm and the signing key id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    pub algo: SignAlgo,
    pub key_id: KeyId,
    pub bytes: Vec<u8>,
}

/// Hashing strategy. Pure computation (no I/O), so it belongs to the domain
/// side of the dependency rule even though concrete impls live here.
pub trait Hasher: Send + Sync {
    fn algo(&self) -> HashAlgo;
    fn hash(&self, data: &[u8]) -> Digest;
}

/// Signing strategy. Pure given the key material; *loading* the key is I/O and
/// belongs to a `KeyStore` port in the consuming bounded context.
pub trait Signer: Send + Sync {
    fn algo(&self) -> SignAlgo;
    fn key_id(&self) -> KeyId;
    fn sign(&self, data: &[u8]) -> Signature;
}

/// Verification strategy (public-key side).
pub trait Verifier: Send + Sync {
    fn algo(&self) -> SignAlgo;
    fn verify(&self, data: &[u8], sig: &Signature) -> bool;
}

/// Errors raised when an algorithm is requested but not compiled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    UnsupportedHash(HashAlgo),
    UnsupportedSignature(SignAlgo),
    InvalidKey(String),
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CryptoError::UnsupportedHash(a) => {
                write!(
                    f,
                    "hash algorithm {a:?} is not enabled (check cargo features)"
                )
            }
            CryptoError::UnsupportedSignature(a) => {
                write!(
                    f,
                    "signature algorithm {a:?} is not enabled (check cargo features)"
                )
            }
            CryptoError::InvalidKey(msg) => write!(f, "invalid key: {msg}"),
        }
    }
}

impl std::error::Error for CryptoError {}

/// Shared `Arc<dyn Hasher>` handle, the form consumers inject at the
/// composition root.
pub type SharedHasher = Arc<dyn Hasher>;
