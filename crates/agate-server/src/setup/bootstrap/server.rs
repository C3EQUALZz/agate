use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::warn;

use agate_audit::application::common::ports::AuditMetrics;
use agate_audit::domain::merkle::LogId;
use agate_audit::infrastructure::AuditMetricsRecorder;
use agate_audit::setup::ioc::{build_container, build_registry};
use agate_policy::application::PolicyService;
use agate_policy::domain::decision::PolicyRuleset;
use agate_proxy::application::common::ports::{AuditSink, PolicyPort};
use agate_proxy::setup::bootstrap::build_app_with;
use agate_proxy::setup::configs::ProxyConfig;

use crate::infrastructure::audit::{AuditLogSink, AuditOutbox};
use crate::infrastructure::policy::PolicyAdapter;

/// How many inspected records may queue before the forwarding path feels
/// backpressure from the audit write. Bounded so a slow database cannot grow
/// memory without limit.
const OUTBOX_CAPACITY: usize = 1024;

/// The wired server: the proxy HTTP app to serve, and the audit outbox task
/// draining records into the transparency log.
///
/// On graceful shutdown the caller serves `app` with an axum shutdown signal;
/// once `serve` returns, dropping `app` drops the audit sink and closes the
/// outbox channel, so awaiting `outbox` flushes the records still queued before
/// the process exits (see `main`).
pub struct Server {
    pub app: Router,
    pub outbox: JoinHandle<()>,
}

/// Wire the proxy to the policy `ruleset` and the audit log identified by `log`,
/// backed by `pool`.
///
/// Policy decisions come from `agate-policy` (via [`PolicyAdapter`]); the audit
/// sink is the real bridge to the transparency log. Must be called from within a
/// Tokio runtime — it spawns the outbox task.
#[must_use]
pub fn build_server(
    proxy: ProxyConfig,
    pool: PgPool,
    log: LogId,
    ruleset: PolicyRuleset,
) -> Server {
    let container = build_container(pool.clone());
    let registry = Arc::new(build_registry());
    let metrics: Arc<dyn AuditMetrics> = Arc::new(AuditMetricsRecorder);

    let (tx, rx) = mpsc::channel::<Vec<u8>>(OUTBOX_CAPACITY);
    let outbox = tokio::spawn(AuditOutbox::new(container, registry, log, metrics.clone()).run(rx));

    let policy: Arc<dyn PolicyPort> = Arc::new(PolicyAdapter::new(PolicyService::new(ruleset)));
    let audit: Arc<dyn AuditSink> = Arc::new(AuditLogSink::new(tx, metrics));
    let app = build_app_with(proxy, policy, audit).merge(readiness_router(pool));

    Server { app, outbox }
}

/// A `/readyz` route that reports readiness from the database's health. Liveness
/// (`/healthz`, served by the proxy) only says the process is up; readiness adds
/// "can it reach its dependencies" so an orchestrator holds traffic until the
/// transparency-log store is reachable.
fn readiness_router(pool: PgPool) -> Router {
    Router::new().route("/readyz", get(readyz)).with_state(pool)
}

/// 200 when a database connection can be acquired, 503 otherwise.
async fn readyz(State(pool): State<PgPool>) -> Response {
    match pool.acquire().await {
        Ok(_connection) => (StatusCode::OK, "ready").into_response(),
        Err(error) => {
            warn!(%error, "readiness probe failed: database unavailable");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "not ready: database unavailable",
            )
                .into_response()
        }
    }
}
