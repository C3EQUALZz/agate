//! The connected `Storage` backend against a real database: `connect` runs
//! migrations and the readiness `HealthCheck` reports the live pool healthy.

use agate_audit::setup::configs::{PostgresConfig, StorageConfig};
use agate_audit::setup::storage::Storage;

use crate::fixture::start_without_migrations;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connect_runs_migrations_and_reports_healthy() {
    let db = start_without_migrations().await;
    assert!(
        !checkpoint_table_exists(&db.pool).await,
        "precondition: the migration-free fixture has no schema yet"
    );

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

    assert!(
        checkpoint_table_exists(&db.pool).await,
        "Storage::connect applied the migrations"
    );
}

/// Whether the `audit_checkpoint` table (migration 0002) is present.
async fn checkpoint_table_exists(pool: &sqlx::PgPool) -> bool {
    let (exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = 'audit_checkpoint')",
    )
    .fetch_one(pool)
    .await
    .expect("query the schema");
    exists
}
