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

/// The slot a request scope's transaction lives in: `None` until `begin`,
/// `Some` while open. The `'static` lifetime detaches it from the borrowed
/// pool so it can be shared. Registered as-is in the IoC container; the
/// container's `Arc` wrapper is exactly a [`SharedTransaction`].
pub type TxSlot = Mutex<Option<Transaction<'static, Postgres>>>;

/// A single transaction shared by every gateway of one request scope.
pub type SharedTransaction = Arc<TxSlot>;

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
