use std::sync::Arc;

use crate::application::common::ports::SignatureFactory;
use crate::domain::common::errors::CryptoError;
use crate::domain::common::values::SecretKey;
use crate::domain::signing::{KeyId, SignAlgo, Signer, Verifier};

#[cfg(feature = "ed25519")]
use super::ed25519::{Ed25519Signer, Ed25519Verifier};

/// Concrete [`SignatureFactory`] backed by RustCrypto signature crates.
pub struct RustCryptoSignatureFactory;

impl SignatureFactory for RustCryptoSignatureFactory {
    #[cfg_attr(not(feature = "ed25519"), allow(unused_variables))]
    fn signer(
        &self,
        algo: SignAlgo,
        secret: &SecretKey,
        key_id: KeyId,
    ) -> Result<Arc<dyn Signer>, CryptoError> {
        match algo {
            #[cfg(feature = "ed25519")]
            SignAlgo::Ed25519 => {
                let seed: [u8; 32] = secret.expose().try_into().map_err(|_| {
                    CryptoError::InvalidKey("Ed25519 seed must be 32 bytes".to_string())
                })?;
                Ok(Arc::new(Ed25519Signer::from_seed(&seed, key_id)))
            }
            _ => Err(CryptoError::UnsupportedSignature(algo)),
        }
    }

    #[cfg_attr(not(feature = "ed25519"), allow(unused_variables))]
    fn verifier(
        &self,
        algo: SignAlgo,
        public_key: &[u8],
    ) -> Result<Arc<dyn Verifier>, CryptoError> {
        match algo {
            #[cfg(feature = "ed25519")]
            SignAlgo::Ed25519 => {
                let bytes: [u8; 32] = public_key.try_into().map_err(|_| {
                    CryptoError::InvalidKey("Ed25519 public key must be 32 bytes".to_string())
                })?;
                Ok(Arc::new(Ed25519Verifier::from_public_bytes(&bytes)?))
            }
            _ => Err(CryptoError::UnsupportedSignature(algo)),
        }
    }
}
