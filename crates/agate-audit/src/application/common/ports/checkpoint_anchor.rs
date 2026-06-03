use async_trait::async_trait;

use crate::application::errors::AuditError;
use crate::domain::merkle::SignedTreeHead;

/// Publishes a signed tree head to an external, independent witness/anchor —
/// the defense against split-view/equivocation by the log operator.
#[async_trait]
pub trait CheckpointAnchor: Send + Sync {
    async fn anchor(&self, sth: &SignedTreeHead) -> Result<(), AuditError>;
}
