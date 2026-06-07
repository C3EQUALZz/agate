use async_trait::async_trait;

use crate::application::errors::AuditError;
use crate::domain::merkle::{LogId, SignedTreeHead};

/// Anchors a signed tree head durably (and, ultimately, to an external
/// independent witness) — the defense against split-view/equivocation by the
/// log operator. `log` identifies which transparency log the checkpoint is for.
#[async_trait]
pub trait CheckpointAnchor: Send + Sync {
    async fn anchor(&self, log: LogId, sth: &SignedTreeHead) -> Result<(), AuditError>;
}
