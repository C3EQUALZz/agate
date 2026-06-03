use std::sync::Arc;

use super::digest_hasher::{DigestHasher, MakeDigest};
use crate::application::common::ports::HasherFactory;
use crate::domain::common::errors::CryptoError;
use crate::domain::hashing::{HashAlgo, Hasher};

/// Resolve `algo` to a boxed digest constructor, honoring enabled features.
#[cfg_attr(
    not(any(feature = "sha2", feature = "sha3", feature = "streebog")),
    allow(unused_variables, unreachable_code)
)]
fn resolve(algo: HashAlgo) -> Result<Arc<dyn Hasher>, CryptoError> {
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
    Ok(Arc::new(DigestHasher::new(algo, make)))
}

/// Concrete [`HasherFactory`] backed by RustCrypto digests.
pub struct RustCryptoHasherFactory;

impl HasherFactory for RustCryptoHasherFactory {
    fn hasher(&self, algo: HashAlgo) -> Result<Arc<dyn Hasher>, CryptoError> {
        resolve(algo)
    }
}

/// Backward-compatible facade for existing call sites (`CryptoRegistry::hasher`).
/// New code should depend on the [`HasherFactory`] port and inject
/// [`RustCryptoHasherFactory`] at the composition root.
pub struct CryptoRegistry;

impl CryptoRegistry {
    pub fn hasher(algo: HashAlgo) -> Result<Arc<dyn Hasher>, CryptoError> {
        resolve(algo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        let h = CryptoRegistry::hasher(HashAlgo::Sha256).unwrap();
        let d = h.hash(b"abc");
        assert_eq!(d.algo, HashAlgo::Sha256);
        assert_eq!(
            d.to_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn factory_and_facade_agree() {
        let factory = RustCryptoHasherFactory;
        let a = factory.hasher(HashAlgo::Sha256).unwrap().hash(b"abc");
        let b = CryptoRegistry::hasher(HashAlgo::Sha256)
            .unwrap()
            .hash(b"abc");
        assert_eq!(a, b);
    }

    #[test]
    fn unsupported_algorithm_reports_error() {
        #[cfg(not(feature = "sha3"))]
        {
            let err = CryptoRegistry::hasher(HashAlgo::Sha3_256).err();
            assert_eq!(err, Some(CryptoError::UnsupportedHash(HashAlgo::Sha3_256)));
        }
    }
}
