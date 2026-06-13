use async_trait::async_trait;

use agate_audit::domain::merkle::{LogId, SignedTreeHead, TreeSize};

use super::scope::ScopeError;

/// Issues one signed checkpoint (STH) for a log, in its own audit request scope
/// (one transaction). `previous_size` lets the caller skip re-anchoring a tree
/// that has not grown since its last checkpoint.
///
/// The scope lifecycle is a composition concern, so the implementation lives at
/// the composition root (`setup`); the scheduler stays container-agnostic — the
/// same split as [`RecordAppender`](super::RecordAppender).
#[async_trait]
pub trait CheckpointIssuer: Send + Sync {
    /// Issue a signed checkpoint for `log`. `previous_size` skips re-anchoring
    /// when the tree has not grown since the caller's last checkpoint.
    async fn issue(
        &self,
        log: LogId,
        previous_size: Option<TreeSize>,
    ) -> Result<SignedTreeHead, ScopeError>;
}
