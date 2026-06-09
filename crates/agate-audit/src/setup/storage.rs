//! The connected persistence backend.
//!
//! [`Storage`] owns the live driver resources (a `PgPool` for Postgres) and is
//! the single place that connects + runs setup/migrations per backend. The
//! composition root builds one from [`StorageConfig`] and hands it to
//! [`build_container`](super::ioc::build_container) and the readiness probe.

use std::sync::Arc;

use sqlx::PgPool;

use crate::application::common::ports::HealthCheck;
use crate::application::errors::AuditError;
use crate::infrastructure::persistence::postgres::{PgHealthCheck, connect_pool, run_migrations};
use crate::setup::configs::StorageConfig;

/// A connected persistence backend: the live resources behind the chosen store.
#[non_exhaustive]
pub enum Storage {
    /// PostgreSQL connection pool.
    Postgres(PgPool),
}

impl Storage {
    /// Wrap an already-connected Postgres pool (e.g. one a test fixture built).
    /// Most callers use [`connect`](Self::connect) instead.
    #[must_use]
    pub fn postgres(pool: PgPool) -> Self {
        Self::Postgres(pool)
    }

    /// Connect to the backend described by `config` and run its schema
    /// setup/migrations. The one async, fallible entry point for the store.
    pub async fn connect(config: &StorageConfig) -> Result<Self, AuditError> {
        match config {
            StorageConfig::Postgres(postgres) => {
                let pool = connect_pool(postgres.url(), postgres.pool()).await?;
                run_migrations(&pool).await?;
                Ok(Self::Postgres(pool))
            }
        }
    }

    /// The readiness probe for this backend, behind the [`HealthCheck`] port so
    /// the `/readyz` route never names a concrete store.
    #[must_use]
    pub fn health_check(&self) -> Arc<dyn HealthCheck> {
        match self {
            Self::Postgres(pool) => Arc::new(PgHealthCheck::new(pool.clone())),
        }
    }
}
