use async_trait::async_trait;

use crate::application::errors::AuditError;
use crate::domain::merkle::{LeafIndex, LogId, TransparencyLog};

/// Write-side gateway: loads and persists the `TransparencyLog` aggregate
/// (the source of truth, e.g. the append-only store).
#[async_trait]
pub trait LogCommandGateway: Send + Sync {
    async fn load(&self, id: LogId) -> Result<Option<TransparencyLog>, AuditError>;
    async fn save(&self, log: &TransparencyLog) -> Result<(), AuditError>;

    /// Append one record's leaf to the log in place and return its index, without
    /// loading or rewriting the existing leaves — the hot path. `load`/`save`
    /// rebuild the whole aggregate (needed to compute a root for a checkpoint, or
    /// a proof), which is `O(n)` per call; appending one leaf must not be, or the
    /// write path is `O(n²)` in the log's size. Returns `None` if the log is
    /// absent. Append-only and single-writer (the audit outbox), so assigning the
    /// next index is race-free.
    async fn append_record(
        &self,
        id: LogId,
        record: &[u8],
    ) -> Result<Option<LeafIndex>, AuditError>;
}
