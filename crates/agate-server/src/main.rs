//! Runs the full Agate server: an inspecting reverse proxy that records every
//! decision to the audit transparency log.
//!
//! Configuration is loaded from `agate.toml` (path from `AGATE_CONFIG`, default
//! `/etc/agate/agate.toml`) layered with `AGATE__SECTION__KEY` environment
//! overrides — see `agate.example.toml`. `AUDIT_LOG_ID` (a UUID) optionally
//! pins the transparency log; when unset a fresh log is created on startup.

use std::sync::Arc;

use sqlx::PgPool;
use uuid::Uuid;

use agate_audit::application::common::messaging::Dispatcher;
use agate_audit::application::usecases::create_log::CreateLog;
use agate_audit::domain::merkle::LogId;
use agate_audit::infrastructure::persistence::postgres::{connect_pool, run_migrations};
use agate_audit::setup::ioc::{build_container, build_registry};
use agate_server::setup::bootstrap::build_server;
use agate_server::setup::configs::load;
use agate_server::setup::observability::{init_logging, init_metrics};
use tracing::info;

#[tokio::main]
async fn main() {
    let config = load().expect("load configuration");
    config
        .validate()
        .unwrap_or_else(|error| panic!("invalid configuration: {error}"));
    let tracer_provider =
        init_logging(&config.observability.logging, &config.observability.tracing);
    if init_metrics(&config.observability.metrics) {
        info!(bind = %config.observability.metrics.bind, "Prometheus metrics endpoint serving /metrics");
    }

    // Build everything that can fail from config before any I/O, so a bad config
    // aborts startup before connecting to Postgres or creating a log.
    let proxy = config.proxy_config();
    let bind_addr = proxy.bind_addr.clone();
    let ruleset = config
        .policy_ruleset()
        .expect("invalid policy configuration");
    let postgres = config.postgres_config();
    let pinned_log_id = std::env::var("AUDIT_LOG_ID")
        .ok()
        .map(|raw| LogId(raw.parse::<Uuid>().expect("AUDIT_LOG_ID must be a UUID")));

    info!("configuration loaded; starting agate-server");

    let pool = connect_pool(postgres.url(), postgres.pool())
        .await
        .expect("connect to Postgres");
    run_migrations(&pool).await.expect("run migrations");
    info!("connected to Postgres and applied migrations");

    let log = resolve_log(&pool, pinned_log_id).await;
    info!(log = %log.0, "recording to transparency log");
    let server = build_server(
        proxy,
        pool,
        log,
        ruleset,
        config.policy_fail_mode(),
        config.policy_decision_timeout(),
    );

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("bind listener");
    info!(%bind_addr, "agate-server listening");

    axum::serve(listener, server.app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve");

    // axum::serve has returned, so the served app — and the audit sink inside it —
    // is dropped, closing the outbox channel. Awaiting the outbox task lets it
    // drain the queued records before the process exits.
    info!("draining the audit outbox");
    server.outbox.await.expect("audit outbox task");

    // Flush any spans still buffered in the OTLP batch exporter before exit.
    if let Some(provider) = tracer_provider
        && let Err(error) = provider.shutdown()
    {
        tracing::warn!(%error, "failed to flush the OTLP tracer on shutdown");
    }
    info!("shutdown complete");
}

/// Resolves once the process receives SIGINT (Ctrl+C) or SIGTERM (the signal a
/// container runtime sends to stop), triggering an axum graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install the Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install the SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    info!("shutdown signal received; stopping new work");
}

/// The transparency log to record into: `AUDIT_LOG_ID` if set, else a freshly
/// created log (so a first run is self-contained; set the env var to keep
/// appending to the same log across restarts).
async fn resolve_log(pool: &PgPool, pinned: Option<LogId>) -> LogId {
    if let Some(id) = pinned {
        return id;
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
