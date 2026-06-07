use async_trait::async_trait;
use sqlx::PgPool;

use crate::application::common::ports::HealthCheck;
use crate::application::errors::AuditError;

use super::storage_error;

/// PostgreSQL-backed [`HealthCheck`]: healthy when a pooled connection can be
/// acquired (which round-trips to the server), unhealthy otherwise.
pub struct PgHealthCheck {
    pool: PgPool,
}

impl PgHealthCheck {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl HealthCheck for PgHealthCheck {
    async fn check(&self) -> Result<(), AuditError> {
        self.pool.acquire().await.map(drop).map_err(storage_error)
    }
}
