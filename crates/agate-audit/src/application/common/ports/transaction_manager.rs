use async_trait::async_trait;

use crate::application::errors::AuditError;

/// Application port: the transaction boundary for one interactor. Gateways
/// enroll their writes in the ambient transaction; `commit` flushes them
/// atomically, `rollback` discards them.
///
/// This is *not* a change-tracking Unit of Work (no `register_dirty`); if that
/// becomes necessary, a `UnitOfWork` can be layered on top.
#[async_trait]
pub trait TransactionManager: Send + Sync {
    async fn commit(&self) -> Result<(), AuditError>;
    async fn rollback(&self) -> Result<(), AuditError>;
}
