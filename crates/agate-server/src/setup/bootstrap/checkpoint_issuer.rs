use std::sync::Arc;

use async_trait::async_trait;
use froodi::async_impl::Container;

use agate_audit::application::common::messaging::Registry;
use agate_audit::application::usecases::issue_checkpoint::IssueCheckpoint;
use agate_audit::domain::merkle::{LogId, SignedTreeHead, TreeSize};
use agate_crypto::KeyId;

use super::scope::ScopedDispatcher;
use crate::infrastructure::audit::{CheckpointIssuer, ScopeError};

/// The composition-root [`CheckpointIssuer`]: issues each checkpoint in its own
/// audit request scope (one transaction), signing with the configured `key`.
/// Knowing the container, the scope lifecycle, and the key id is exactly what
/// the scheduler must not.
pub struct ScopedIssuer {
    dispatcher: ScopedDispatcher,
    key: KeyId,
}

impl ScopedIssuer {
    /// Build an issuer over the audit `container`/`registry`, signing with `key`.
    #[must_use]
    pub fn new(container: Container, registry: Arc<Registry<Container>>, key: KeyId) -> Self {
        Self {
            dispatcher: ScopedDispatcher::new(container, registry),
            key,
        }
    }
}

#[async_trait]
impl CheckpointIssuer for ScopedIssuer {
    async fn issue(
        &self,
        log: LogId,
        previous_size: Option<TreeSize>,
    ) -> Result<SignedTreeHead, ScopeError> {
        self.dispatcher
            .dispatch::<IssueCheckpoint, SignedTreeHead>(IssueCheckpoint {
                log,
                key: self.key.clone(),
                previous_size,
            })
            .await
    }
}
