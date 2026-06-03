use async_trait::async_trait;

use crate::application::errors::AuditError;
use crate::domain::merkle::AuditEvent;

/// Persists domain events for reliable later dispatch (transactional outbox).
#[async_trait]
pub trait EventOutbox: Send + Sync {
    async fn publish(&self, events: Vec<AuditEvent>) -> Result<(), AuditError>;
}
