//! PostgreSQL backend shared across this context's aggregates: the
//! request-scoped transaction, its manager, and migrations.
//!
//! The transaction is owned here, not by the gateways. Gateways run their
//! statements on the shared transaction; only `PgTransactionManager` commits
//! or rolls it back, so the commit boundary lives in one place.

use std::sync::Arc;
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use tokio::sync::Mutex;
use tracing::warn;

use crate::application::errors::AuditError;

pub mod health;
pub mod transaction_manager;

pub use health::PgHealthCheck;
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

/// Connection-pool sizing plus the startup connect-retry policy.
///
/// The retry lets a brief database unavailability at boot — the DB container
/// still starting next to ours under compose/Kubernetes — be ridden out with
/// backoff instead of crashing the process on the first failed connect.
#[derive(Clone, Debug)]
pub struct PoolConfig {
    /// Maximum number of pooled connections.
    pub max_connections: u32,
    /// How long `acquire` waits for a free connection before erroring.
    pub acquire_timeout: Duration,
    /// How many times to retry the initial connect before giving up
    /// (`0` = try once, no retry).
    pub connect_max_retries: u32,
    /// Base delay between connect attempts; doubled each retry up to a cap.
    pub connect_backoff: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 10,
            acquire_timeout: Duration::from_secs(30),
            connect_max_retries: 10,
            connect_backoff: Duration::from_secs(1),
        }
    }
}

/// Connect to Postgres with the given pool sizing, retrying the initial
/// connection with capped exponential backoff. Returns the last error once the
/// retry budget (`connect_max_retries`) is exhausted.
pub async fn connect_pool(url: &str, config: &PoolConfig) -> Result<PgPool, AuditError> {
    let options = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .acquire_timeout(config.acquire_timeout);

    let mut attempt: u32 = 0;
    loop {
        match options.clone().connect(url).await {
            Ok(pool) => return Ok(pool),
            Err(error) => {
                if attempt >= config.connect_max_retries {
                    return Err(storage_error(error));
                }
                // Cap the exponent so the backoff plateaus on a long outage
                // rather than growing without bound.
                let backoff = config.connect_backoff * 2_u32.pow(attempt.min(6));
                warn!(
                    attempt = attempt + 1,
                    retries = config.connect_max_retries,
                    ?backoff,
                    %error,
                    "Postgres connection failed; retrying after backoff"
                );
                tokio::time::sleep(backoff).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PoolConfig, connect_pool};
    use std::time::Duration;

    // Port 1 on loopback refuses immediately, so with no retries this returns
    // the connect error promptly instead of hanging.
    #[tokio::test]
    async fn connect_pool_gives_up_after_the_retry_budget() {
        let config = PoolConfig {
            max_connections: 1,
            acquire_timeout: Duration::from_millis(50),
            connect_max_retries: 0,
            connect_backoff: Duration::from_millis(1),
        };
        let result = connect_pool("postgres://agate@127.0.0.1:1/agate", &config).await;
        assert!(result.is_err());
    }

    // With a retry budget, the backoff branch runs (sleep + retry) before the
    // final give-up — exercises the loop, not just the immediate-failure path.
    #[tokio::test]
    async fn connect_pool_retries_then_gives_up() {
        let config = PoolConfig {
            max_connections: 1,
            acquire_timeout: Duration::from_millis(50),
            connect_max_retries: 2,
            connect_backoff: Duration::from_millis(1),
        };
        let result = connect_pool("postgres://agate@127.0.0.1:1/agate", &config).await;
        assert!(result.is_err());
    }
}
