//! PostgreSQL backend shared across this context's aggregates: the
//! request-scoped transaction, its manager, and migrations.
//!
//! The transaction is owned here, not by the gateways. Gateways run their
//! statements on the shared transaction; only `PgTransactionManager` commits
//! or rolls it back, so the commit boundary lives in one place.

use std::sync::Arc;

use sqlx::{PgPool, Postgres, Transaction};
use tokio::sync::Mutex;

use crate::application::errors::AuditError;

pub mod transaction_manager;

pub use transaction_manager::PgTransactionManager;

/// A single transaction shared by every gateway of one request scope.
///
/// `None` until `begin`; `Some` while a transaction is open. The `'static`
/// lifetime detaches it from the borrowed pool so it can live in an `Arc`.
pub type SharedTransaction = Arc<Mutex<Option<Transaction<'static, Postgres>>>>;

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
