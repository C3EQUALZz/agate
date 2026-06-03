use std::sync::Arc;

use crate::domain::common::errors::CryptoError;
use crate::domain::common::values::SecretKey;
use crate::domain::signing::{KeyId, SignAlgo, Signer, Verifier};

/// Abstract factory building [`Signer`] / [`Verifier`] strategies from supplied
/// key material. Key *loading* is a consumer concern (a `KeyStore` port in the
/// owning context); this factory only adapts bytes into a strategy.
pub trait SignatureFactory: Send + Sync {
    fn signer(
        &self,
        algo: SignAlgo,
        secret: &SecretKey,
        key_id: KeyId,
    ) -> Result<Arc<dyn Signer>, CryptoError>;

    fn verifier(&self, algo: SignAlgo, public_key: &[u8])
    -> Result<Arc<dyn Verifier>, CryptoError>;
}
