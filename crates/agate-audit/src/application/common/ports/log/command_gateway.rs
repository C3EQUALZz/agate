use async_trait::async_trait;

use crate::application::errors::AuditError;
use crate::domain::merkle::{LogId, TransparencyLog};

/// Write-side gateway: loads and persists the `TransparencyLog` aggregate
/// (the source of truth, e.g. the append-only store).
#[async_trait]
pub trait LogCommandGateway: Send + Sync {
    async fn load(&self, id: LogId) -> Result<Option<TransparencyLog>, AuditError>;
    async fn save(&self, log: &TransparencyLog) -> Result<(), AuditError>;
}
