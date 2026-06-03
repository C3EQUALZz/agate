use std::fmt;

use crate::domain::encryption::AeadAlgo;
use crate::domain::hashing::HashAlgo;
use crate::domain::signing::SignAlgo;

/// Root of the crypto error hierarchy.
///
/// Cipher failures are deliberately opaque ([`Encryption`](Self::Encryption) /
/// [`Decryption`](Self::Decryption)) so that adapters cannot leak oracle-style
/// detail (e.g. padding vs. tag mismatch) to a caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    UnsupportedHash(HashAlgo),
    UnsupportedSignature(SignAlgo),
    UnsupportedAead(AeadAlgo),
    InvalidKey(String),
    InvalidNonce(String),
    Encryption,
    Decryption,
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
            CryptoError::UnsupportedAead(a) => {
                write!(
                    f,
                    "AEAD algorithm {a:?} is not enabled (check cargo features)"
                )
            }
            CryptoError::InvalidKey(msg) => write!(f, "invalid key: {msg}"),
            CryptoError::InvalidNonce(msg) => write!(f, "invalid nonce: {msg}"),
            CryptoError::Encryption => write!(f, "encryption failed"),
            CryptoError::Decryption => write!(f, "decryption failed (authentication error)"),
        }
    }
}

impl std::error::Error for CryptoError {}
