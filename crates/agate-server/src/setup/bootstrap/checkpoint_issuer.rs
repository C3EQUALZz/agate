use std::sync::Arc;

use async_trait::async_trait;
use froodi::async_impl::Container;

use agate_audit::application::common::messaging::{Dispatcher, Registry};
use agate_audit::application::usecases::issue_checkpoint::IssueCheckpoint;
use agate_audit::domain::merkle::{LogId, SignedTreeHead, TreeSize};
use agate_crypto::KeyId;

use crate::infrastructure::audit::{CheckpointIssuer, IssueError};

/// The composition-root [`CheckpointIssuer`]: opens one audit request scope per
/// issue (one transaction) and dispatches [`IssueCheckpoint`] through the
/// pipeline, signing with the configured `key`. Knowing the container, the
/// scope lifecycle, and the key id is exactly what the scheduler must not.
pub struct ScopedIssuer {
    container: Container,
    registry: Arc<Registry<Container>>,
    key: KeyId,
}

impl ScopedIssuer {
    #[must_use]
    pub fn new(container: Container, registry: Arc<Registry<Container>>, key: KeyId) -> Self {
        Self {
            container,
            registry,
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
    ) -> Result<SignedTreeHead, IssueError> {
        let scope = self
            .container
            .clone()
            .enter_build()
            .map_err(|error| IssueError::ScopeUnavailable(format!("{error:?}")))?;
        let scope = Arc::new(scope);
        let dispatcher = Dispatcher::new(scope.clone(), self.registry.clone());
        let result = dispatcher
            .send(IssueCheckpoint {
                log,
                key: self.key.clone(),
                previous_size,
            })
            .await;
        scope.close().await;
        result.map_err(IssueError::Pipeline)
    }
}
