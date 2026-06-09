//! Runs the audit HTTP service. Configure with `DATABASE_URL` (required) and
//! `BIND_ADDR` (default `0.0.0.0:8080`).

use agate_audit::setup::bootstrap::build_app;
use agate_audit::setup::configs::{AppConfig, StorageConfig};
use agate_audit::setup::storage::Storage;

#[tokio::main]
async fn main() {
    let config = AppConfig::from_env();

    let storage = Storage::connect(&StorageConfig::Postgres(config.postgres.clone()))
        .await
        .expect("connect to the transparency-log store");

    let listener = tokio::net::TcpListener::bind(&config.http.bind_addr)
        .await
        .expect("bind listener");
    axum::serve(listener, build_app(&storage))
        .await
        .expect("serve");
}
