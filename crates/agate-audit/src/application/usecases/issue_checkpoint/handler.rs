use std::sync::Arc;

use async_trait::async_trait;

use super::command::IssueCheckpoint;
use crate::application::common::messaging::RequestHandler;
use crate::application::common::ports::{CheckpointAnchor, KeyStore, LogCommandGateway};
use crate::application::errors::AuditError;
use crate::domain::merkle::{CheckpointSigner, SignedTreeHead};
use crate::domain::ports::Clock;

pub struct IssueCheckpointHandler {
    gateway: Arc<dyn LogCommandGateway>,
    keys: Arc<dyn KeyStore>,
    anchor: Arc<dyn CheckpointAnchor>,
    clock: Arc<dyn Clock>,
}

impl IssueCheckpointHandler {
    pub fn new(
        gateway: Arc<dyn LogCommandGateway>,
        keys: Arc<dyn KeyStore>,
        anchor: Arc<dyn CheckpointAnchor>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            gateway,
            keys,
            anchor,
            clock,
        }
    }
}

#[async_trait]
impl RequestHandler<IssueCheckpoint> for IssueCheckpointHandler {
    async fn handle(&self, request: IssueCheckpoint) -> Result<SignedTreeHead, AuditError> {
        let mut log = self
            .gateway
            .load(request.log)
            .await?
            .ok_or(AuditError::LogNotFound(request.log))?;

        let signer = self.keys.signer(&request.key).await?;

        // Idle skip: the tree has not grown since the caller's last checkpoint,
        // so the checkpoint at this size is already recorded and anchored. Sign
        // and return the current head, but record no redundant event, re-anchor
        // nothing, and don't save.
        if request.previous_size == Some(log.size()) {
            let head = log.head(self.clock.now());
            return Ok(CheckpointSigner::sign(signer.as_ref(), &head));
        }

        // Snapshot the head (records a CheckpointIssued domain event).
        let head = log.issue_checkpoint(self.clock.now());

        // Sign with the requested key, then publish externally before persisting.
        let sth = CheckpointSigner::sign(signer.as_ref(), &head);
        self.anchor.anchor(request.log, &sth).await?;

        self.gateway.save(&log).await?;
        Ok(sth)
    }
}
