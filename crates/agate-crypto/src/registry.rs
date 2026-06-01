//! Algorithm registry: resolves a self-describing [`HashAlgo`] to a concrete
//! [`Hasher`] implementation, honoring the cargo features that are enabled.

use std::sync::Arc;

use crate::{CryptoError, Digest, HashAlgo, Hasher};

/// Entry point for resolving algorithms to implementations at the composition
/// root (driven by user configuration).
pub struct CryptoRegistry;

impl CryptoRegistry {
    /// Resolve a hasher for `algo`, or [`CryptoError::UnsupportedHash`] if the
    /// corresponding cargo feature is not enabled.
    pub fn hasher(algo: HashAlgo) -> Result<Arc<dyn Hasher>, CryptoError> {
        let make: MakeDigest = match algo {
            #[cfg(feature = "sha2")]
            HashAlgo::Sha256 => || Box::new(sha2::Sha256::default()),
            #[cfg(feature = "sha2")]
            HashAlgo::Sha512 => || Box::new(sha2::Sha512::default()),
            #[cfg(feature = "sha3")]
            HashAlgo::Sha3_256 => || Box::new(sha3::Sha3_256::default()),
            #[cfg(feature = "streebog")]
            HashAlgo::Streebog256 => || Box::new(streebog::Streebog256::default()),
            #[cfg(feature = "streebog")]
            HashAlgo::Streebog512 => || Box::new(streebog::Streebog512::default()),
            #[allow(unreachable_patterns)]
            _ => return Err(CryptoError::UnsupportedHash(algo)),
        };
        Ok(Arc::new(DigestHasher { algo, make }))
    }
}

/// Factory for a fresh boxed digest state. A non-capturing closure, so it
/// coerces to a plain function pointer.
type MakeDigest = fn() -> Box<dyn digest::DynDigest>;

/// Adapts any RustCrypto `DynDigest` into our [`Hasher`] strategy.
struct DigestHasher {
    algo: HashAlgo,
    make: MakeDigest,
}

impl Hasher for DigestHasher {
    fn algo(&self) -> HashAlgo {
        self.algo
    }

    fn hash(&self, data: &[u8]) -> Digest {
        let mut state = (self.make)();
        state.update(data);
        Digest {
            algo: self.algo,
            bytes: state.finalize().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        // SHA-256("abc")
        let h = CryptoRegistry::hasher(HashAlgo::Sha256).unwrap();
        let d = h.hash(b"abc");
        assert_eq!(d.algo, HashAlgo::Sha256);
        assert_eq!(
            d.to_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn unsupported_algorithm_reports_error() {
        // sha3 feature is off by default.
        #[cfg(not(feature = "sha3"))]
        {
            let err = CryptoRegistry::hasher(HashAlgo::Sha3_256).err();
            assert_eq!(err, Some(CryptoError::UnsupportedHash(HashAlgo::Sha3_256)));
        }
    }
}
