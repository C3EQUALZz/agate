//! Runs the full Agate server: an inspecting reverse proxy that records every
//! decision to the audit transparency log.
//!
//! Configure with `AGENT_ENDPOINT` (required), `DATABASE_URL` (required),
//! `BIND_ADDR` (default `0.0.0.0:8080`), and optionally `AUDIT_LOG_ID` (a UUID);
//! when `AUDIT_LOG_ID` is unset a fresh log is created on startup.

use std::sync::Arc;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use agate_audit::application::common::messaging::Dispatcher;
use agate_audit::application::usecases::create_log::CreateLog;
use agate_audit::domain::merkle::LogId;
use agate_audit::infrastructure::persistence::postgres::run_migrations;
use agate_audit::setup::ioc::{build_container, build_registry};
use agate_server::setup::bootstrap::build_server;
use agate_server::setup::configs::ServerConfig;

#[tokio::main]
async fn main() {
    let config = ServerConfig::from_env();
    let bind_addr = config.proxy.bind_addr.clone();

    let pool = PgPoolOptions::new()
        .connect(config.postgres.url())
        .await
        .expect("connect to Postgres");
    run_migrations(&pool).await.expect("run migrations");

    let log = resolve_log(&pool).await;
    let server = build_server(config.proxy, pool, log);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("bind listener");
    axum::serve(listener, server.app).await.expect("serve");
}

/// The transparency log to record into: `AUDIT_LOG_ID` if set, else a freshly
/// created log (so a first run is self-contained; set the env var to keep
/// appending to the same log across restarts).
async fn resolve_log(pool: &PgPool) -> LogId {
    if let Ok(id) = std::env::var("AUDIT_LOG_ID") {
        let id = id.parse::<Uuid>().expect("AUDIT_LOG_ID must be a UUID");
        return LogId(id);
    }

    let container = build_container(pool.clone());
    let registry = Arc::new(build_registry());
    let scope = Arc::new(container.enter_build().expect("open request scope"));
    let dispatcher = Dispatcher::new(scope.clone(), registry);
    let log = dispatcher
        .send(CreateLog)
        .await
        .expect("create transparency log");
    scope.close().await;
    // Printed (not just traced) so it shows without a subscriber configured:
    // operators need the id to set AUDIT_LOG_ID and keep the same log on restart.
    println!(
        "created transparency log {0}; set AUDIT_LOG_ID={0} to reuse it",
        log.0
    );
    log
}
