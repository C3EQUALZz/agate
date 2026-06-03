//! PostgreSQL adapters for the transparency-log gateways.

use sqlx::PgPool;

use crate::application::errors::AuditError;

pub mod command_gateway;
pub mod query_gateway;

pub use command_gateway::PostgresLogCommandGateway;
pub use query_gateway::PostgresLogQueryGateway;

// Owned arg so it composes as `.map_err(storage_error)`.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn storage_error(err: sqlx::Error) -> AuditError {
    AuditError::Storage(err.to_string())
}

/// Apply the embedded migrations to `pool` (idempotent; tracked by sqlx).
pub async fn run_migrations(pool: &PgPool) -> Result<(), AuditError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|err| AuditError::Storage(err.to_string()))
}
