use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use agate_audit::application::common::ports::AuditMetrics;
use agate_audit::domain::merkle::LogId;
use agate_audit::infrastructure::AuditMetricsRecorder;
use agate_audit::setup::ioc::{build_container, build_registry};
use agate_audit::setup::storage::Storage;
use agate_crypto::KeyId;
use agate_policy::application::PolicyService;
use agate_policy::domain::decision::PolicyRuleset;
use agate_proxy::application::common::ports::{AuditSink, PolicyPort};
use agate_proxy::infrastructure::{FailMode, FailModePolicy};
use agate_proxy::setup::bootstrap::build_app_with;
use agate_proxy::setup::configs::ProxyConfig;

use crate::infrastructure::audit::{
    AuditLogSink, AuditOutbox, CheckpointIssuer, CheckpointScheduler, RecordAppender,
};
use crate::infrastructure::policy::PolicyAdapter;
use crate::presentation::http::readiness;
use crate::setup::bootstrap::{ScopedAppender, ScopedIssuer};

/// How many inspected records may queue before the forwarding path feels
/// backpressure from the audit write. Bounded so a slow database cannot grow
/// memory without limit.
const OUTBOX_CAPACITY: usize = 1024;

/// How a periodic signed checkpoint (STH) is issued for the log.
pub struct CheckpointSettings {
    /// How often to issue.
    pub period: Duration,
    /// The signing key id to request (must match the loaded key store key).
    pub key: KeyId,
}

/// The wired server: the proxy HTTP app to serve, the audit outbox task draining
/// records into the transparency log, and an optional periodic checkpoint task.
///
/// On graceful shutdown the caller serves `app` with an axum shutdown signal;
/// once `serve` returns, dropping `app` drops the audit sink and closes the
/// outbox channel, so awaiting `outbox` flushes the records still queued before
/// the process exits (see `main`). The `checkpoint` task is a timer loop with no
/// natural end, so the caller aborts it on shutdown.
pub struct Server {
    pub app: Router,
    pub outbox: JoinHandle<()>,
    pub checkpoint: Option<JoinHandle<()>>,
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
    storage: &Storage,
    log: LogId,
    ruleset: PolicyRuleset,
    fail_mode: FailMode,
    decision_timeout: Duration,
    checkpoint: Option<CheckpointSettings>,
) -> Server {
    let container = build_container(storage);
    let registry = Arc::new(build_registry());
    let metrics: Arc<dyn AuditMetrics> = Arc::new(AuditMetricsRecorder);

    let appender: Arc<dyn RecordAppender> =
        Arc::new(ScopedAppender::new(container.clone(), registry.clone()));
    let (tx, rx) = mpsc::channel::<Vec<u8>>(OUTBOX_CAPACITY);
    let outbox = tokio::spawn(AuditOutbox::new(appender, log, metrics.clone()).run(rx));

    // The transparency log's own STH cadence: a background task signs the head
    // on an interval (disabled unless configured). It reuses the same audit
    // container/registry as the outbox, behind the CheckpointIssuer port.
    let checkpoint = checkpoint.map(|settings| {
        let issuer: Arc<dyn CheckpointIssuer> =
            Arc::new(ScopedIssuer::new(container, registry, settings.key));
        tokio::spawn(CheckpointScheduler::new(issuer, log, settings.period).run())
    });

    // The real policy, wrapped so a slow/hung decision falls back to the
    // configured fail mode (fail-closed by default) instead of hanging the run.
    let real_policy: Arc<dyn PolicyPort> =
        Arc::new(PolicyAdapter::new(PolicyService::new(ruleset)));
    let policy: Arc<dyn PolicyPort> = Arc::new(FailModePolicy::new(
        real_policy,
        fail_mode,
        decision_timeout,
    ));
    let audit: Arc<dyn AuditSink> = Arc::new(AuditLogSink::new(tx, metrics));

    // Readiness is reported through the store's HealthCheck port, supplied by
    // the connected backend — so swapping the store touches neither this nor the
    // probe route.
    let health = storage.health_check();
    let app = build_app_with(proxy, policy, audit).merge(readiness::router(health));

    Server {
        app,
        outbox,
        checkpoint,
    }
}
