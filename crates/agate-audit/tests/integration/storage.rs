//! The connected `Storage` backend against a real database: `connect` runs
//! migrations and the readiness `HealthCheck` reports the live pool healthy.

use agate_audit::setup::configs::{PostgresConfig, StorageConfig};
use agate_audit::setup::storage::Storage;

use crate::fixture::start;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connect_runs_migrations_and_reports_healthy() {
    let db = start().await;

    let storage = Storage::connect(&StorageConfig::Postgres(PostgresConfig::new(
        db.url.clone(),
    )))
    .await
    .expect("connect to Postgres and run migrations");

    storage
        .health_check()
        .check()
        .await
        .expect("the connected store is reachable");

    // Migrations ran: the checkpoint table from 0002 exists.
    let (exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'audit_checkpoint')",
    )
    .fetch_one(&db.pool)
    .await
    .expect("query the schema");
    assert!(exists, "Storage::connect applied the migrations");
}
