use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use tokio::sync::mpsc;

use agate_audit::application::common::ports::AuditMetrics;
use agate_audit::domain::merkle::LogId;
use agate_audit::infrastructure::AuditMetricsRecorder;
use agate_audit::setup::ioc::{build_container, build_registry};
use agate_audit::setup::storage::Storage;
use agate_crypto::KeyId;
use agate_proxy::application::common::ports::{AuditSink, PolicyPort};
use agate_proxy::infrastructure::{FailMode, FailModePolicy};
use agate_proxy::setup::bootstrap::build_app_with;
use agate_proxy::setup::configs::ProxyConfig;

use crate::infrastructure::audit::{
    AuditLogSink, AuditOutbox, CheckpointIssuer, CheckpointScheduler, FullPolicy, RecordAppender,
};
use crate::presentation::http::readiness;
use crate::setup::bootstrap::{ScopedAppender, ScopedIssuer, Supervisor};

/// How the audit outbox is sized and behaves when full. Bounded so a slow
/// database cannot grow memory without limit; `on_full` is the operator's
/// completeness-vs-availability choice for a saturated queue.
pub struct OutboxSettings {
    /// How many inspected records may queue before the channel is full.
    pub capacity: usize,
    /// What to do when the queue is full: block (backpressure) or shed.
    pub on_full: FullPolicy,
}

/// How a periodic signed checkpoint (STH) is issued for the log.
pub struct CheckpointSettings {
    /// How often to issue.
    pub period: Duration,
    /// The signing key id to request (must match the loaded key store key).
    pub key: KeyId,
}

/// The wired server: the proxy HTTP app to serve. Its background tasks (the
/// audit outbox draining records into the transparency log, and an optional
/// periodic checkpoint signer) are spawned onto the caller's [`Supervisor`], not
/// returned here.
///
/// On graceful shutdown the caller serves `app` with an axum shutdown signal and
/// [`Supervisor::trigger`]s the token; once `serve` returns, dropping `app` drops
/// the audit sink and closes the outbox channel, so the outbox task drains the
/// records still queued. [`Supervisor::wait`] then awaits every background task
/// before the process exits (see `main`). The checkpoint task is a timer loop
/// with no natural end, so it watches the token and returns at its next boundary
/// rather than being aborted mid-issue.
pub struct Server {
    pub app: Router,
}

/// Everything the server is wired from, beyond the connected `storage`: the
/// proxy data-plane config, the transparency log to record into, the policy
/// ruleset, the policy-decision fail mode and deadline, the optional checkpoint
/// cadence, and the audit-outbox sizing/backpressure policy.
pub struct ServerConfig {
    pub proxy: ProxyConfig,
    pub log: LogId,
    /// The decision engine (built by the composition root): the static ruleset
    /// adapter or the CEL adapter, both `PolicyPort`. `build_server` wraps it in
    /// the fail-mode guard; it does not choose the backend.
    pub policy: Arc<dyn PolicyPort>,
    pub fail_mode: FailMode,
    pub decision_timeout: Duration,
    pub checkpoint: Option<CheckpointSettings>,
    pub outbox: OutboxSettings,
}

/// Wire the proxy to the policy engine and the audit log, backed by `storage`.
///
/// The decision engine is supplied already built (see [`ServerConfig::policy`]);
/// the audit sink is the real bridge to the transparency log. The outbox and the
/// optional checkpoint scheduler are spawned onto `supervisor`, so the caller
/// shuts them down (token + wait) uniformly with every other background task.
/// Must be called from within a Tokio runtime.
#[must_use]
pub fn build_server(storage: &Storage, config: ServerConfig, supervisor: &Supervisor) -> Server {
    let ServerConfig {
        proxy,
        log,
        policy: real_policy,
        fail_mode,
        decision_timeout,
        checkpoint,
        outbox,
    } = config;

    let container = build_container(storage);
    let registry = Arc::new(build_registry());
    let metrics: Arc<dyn AuditMetrics> = Arc::new(AuditMetricsRecorder);

    let appender: Arc<dyn RecordAppender> =
        Arc::new(ScopedAppender::new(container.clone(), registry.clone()));
    let (tx, rx) = mpsc::channel::<Vec<u8>>(outbox.capacity);
    // The outbox is driven to completion by its channel closing (when the served
    // app, and the sink inside it, drops), not by the token — so it drains every
    // queued record on shutdown. It is supervised only so `wait` awaits that drain.
    supervisor.spawn(AuditOutbox::new(appender, log, metrics.clone()).run(rx));

    // The transparency log's own STH cadence: a background task signs the head
    // on an interval (disabled unless configured). It reuses the same audit
    // container/registry as the outbox, behind the CheckpointIssuer port, and
    // watches the shutdown token so it returns at a loop boundary (never mid-issue).
    if let Some(settings) = checkpoint {
        let issuer: Arc<dyn CheckpointIssuer> =
            Arc::new(ScopedIssuer::new(container, registry, settings.key));
        supervisor
            .spawn(CheckpointScheduler::new(issuer, log, settings.period).run(supervisor.token()));
    }

    // Wrap the supplied engine so a slow/hung decision falls back to the
    // configured fail mode (fail-closed by default) instead of hanging the run.
    let policy: Arc<dyn PolicyPort> = Arc::new(FailModePolicy::new(
        real_policy,
        fail_mode,
        decision_timeout,
    ));
    let audit: Arc<dyn AuditSink> = Arc::new(AuditLogSink::new(tx, metrics, outbox.on_full));

    // Readiness is reported through the store's HealthCheck port, supplied by
    // the connected backend — so swapping the store touches neither this nor the
    // probe route.
    let health = storage.health_check();
    let app = build_app_with(proxy, policy, audit).merge(readiness::router(health));

    Server { app }
}
