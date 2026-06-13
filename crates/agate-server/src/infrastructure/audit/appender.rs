use async_trait::async_trait;

use agate_audit::domain::merkle::{LeafIndex, LogId};

use super::scope::ScopeError;

/// Appends one record to a transparency log, each append in its own audit
/// request scope (one transaction, the audit context's commit boundary).
///
/// The scope lifecycle is a composition concern, so the implementation lives
/// at the composition root (`setup`); the outbox stays container-agnostic.
#[async_trait]
pub trait RecordAppender: Send + Sync {
    /// Append one encoded `record` to `log`, returning the assigned leaf index.
    async fn append(&self, log: LogId, record: Vec<u8>) -> Result<LeafIndex, ScopeError>;
}
