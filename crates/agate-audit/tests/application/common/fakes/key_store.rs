use std::sync::Arc;

use async_trait::async_trait;

use agate_audit::application::common::ports::KeyStore;
use agate_audit::application::errors::AuditError;
use agate_crypto::ed25519::{Ed25519Signer, Ed25519Verifier};
use agate_crypto::{KeyId, Signer, Verifier};

/// Ed25519 key store fixed to a single seed (test double).
pub struct FakeKeyStore {
    seed: [u8; 32],
}

impl FakeKeyStore {
    pub fn new(seed: [u8; 32]) -> Self {
        Self { seed }
    }
}

#[async_trait]
impl KeyStore for FakeKeyStore {
    async fn signer(&self, key: &KeyId) -> Result<Arc<dyn Signer>, AuditError> {
        Ok(Arc::new(Ed25519Signer::from_seed(&self.seed, key.clone())))
    }

    async fn verifier(&self, _key: &KeyId) -> Result<Arc<dyn Verifier>, AuditError> {
        let signer = Ed25519Signer::from_seed(&self.seed, KeyId("verify".to_string()));
        let verifier = Ed25519Verifier::from_public_bytes(&signer.verifying_key_bytes())
            .map_err(|err| AuditError::Storage(err.to_string()))?;
        Ok(Arc::new(verifier))
    }
}
