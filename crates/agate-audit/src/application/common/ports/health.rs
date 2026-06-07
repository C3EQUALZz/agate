use async_trait::async_trait;

use crate::application::errors::AuditError;

/// Application port: can the backing store be reached right now?
///
/// Backs the server's readiness probe. Keeping it a port means the persistence
/// backend is swappable — moving from PostgreSQL to another store (Redis, ...)
/// replaces only the adapter, not the probe route that depends on this trait.
#[async_trait]
pub trait HealthCheck: Send + Sync {
    /// `Ok(())` when the store is reachable, `Err` describing why it is not.
    async fn check(&self) -> Result<(), AuditError>;
}
