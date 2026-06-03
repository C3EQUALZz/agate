use std::sync::Arc;

use agate_crypto::{KeyId, Signer, Verifier};
use async_trait::async_trait;

use crate::application::errors::AuditError;

/// Loads signing/verifying key material (I/O) and yields crypto strategies.
#[async_trait]
pub trait KeyStore: Send + Sync {
    async fn signer(&self, key: &KeyId) -> Result<Arc<dyn Signer>, AuditError>;
    async fn verifier(&self, key: &KeyId) -> Result<Arc<dyn Verifier>, AuditError>;
}
