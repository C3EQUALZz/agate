use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;
use tracing::warn;

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
    AuditLogSink, AuditOutbox, CheckpointIssuer, CheckpointScheduler, FullPolicy, RecordAppender,
};
use crate::infrastructure::policy::PolicyAdapter;
use crate::presentation::http::readiness;
use crate::setup::bootstrap::{ScopedAppender, ScopedIssuer};

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

/// A running checkpoint scheduler and the signal that stops it gracefully.
///
/// Stopping is cooperative: [`stop`](Self::stop) signals the loop, which returns
/// at its next boundary, so the scheduler never dies in the middle of an issue
/// (which would abandon a half-open audit scope/transaction, as an abrupt
/// `abort()` could).
pub struct CheckpointHandle {
    task: JoinHandle<()>,
    shutdown: Arc<Notify>,
}

impl CheckpointHandle {
    /// Signal the scheduler to stop and wait for it to finish any in-flight
    /// issue and exit.
    pub async fn stop(self) {
        self.shutdown.notify_one();
        if let Err(error) = self.task.await {
            warn!(%error, "checkpoint scheduler task did not stop cleanly");
        }
    }
}

/// The wired server: the proxy HTTP app to serve, the audit outbox task draining
/// records into the transparency log, and an optional periodic checkpoint task.
///
/// On graceful shutdown the caller serves `app` with an axum shutdown signal;
/// once `serve` returns, dropping `app` drops the audit sink and closes the
/// outbox channel, so awaiting `outbox` flushes the records still queued before
/// the process exits (see `main`). The `checkpoint` task is a timer loop with no
/// natural end, so the caller stops it cooperatively on shutdown via
/// [`CheckpointHandle::stop`].
pub struct Server {
    pub app: Router,
    pub outbox: JoinHandle<()>,
    pub checkpoint: Option<CheckpointHandle>,
}

/// Everything the server is wired from, beyond the connected `storage`: the
/// proxy data-plane config, the transparency log to record into, the policy
/// ruleset, the policy-decision fail mode and deadline, the optional checkpoint
/// cadence, and the audit-outbox sizing/backpressure policy.
pub struct ServerConfig {
    pub proxy: ProxyConfig,
    pub log: LogId,
    pub ruleset: PolicyRuleset,
    pub fail_mode: FailMode,
    pub decision_timeout: Duration,
    pub checkpoint: Option<CheckpointSettings>,
    pub outbox: OutboxSettings,
}

/// Wire the proxy to the policy ruleset and the audit log, backed by `storage`.
///
/// Policy decisions come from `agate-policy` (via [`PolicyAdapter`]); the audit
/// sink is the real bridge to the transparency log. Must be called from within a
/// Tokio runtime — it spawns the outbox task.
#[must_use]
pub fn build_server(storage: &Storage, config: ServerConfig) -> Server {
    let ServerConfig {
        proxy,
        log,
        ruleset,
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
    let outbox_task = tokio::spawn(AuditOutbox::new(appender, log, metrics.clone()).run(rx));

    // The transparency log's own STH cadence: a background task signs the head
    // on an interval (disabled unless configured). It reuses the same audit
    // container/registry as the outbox, behind the CheckpointIssuer port.
    let checkpoint = checkpoint.map(|settings| {
        let issuer: Arc<dyn CheckpointIssuer> =
            Arc::new(ScopedIssuer::new(container, registry, settings.key));
        let shutdown = Arc::new(Notify::new());
        let task = tokio::spawn(
            CheckpointScheduler::new(issuer, log, settings.period).run(shutdown.clone()),
        );
        CheckpointHandle { task, shutdown }
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
    let audit: Arc<dyn AuditSink> = Arc::new(AuditLogSink::new(tx, metrics, outbox.on_full));

    // Readiness is reported through the store's HealthCheck port, supplied by
    // the connected backend — so swapping the store touches neither this nor the
    // probe route.
    let health = storage.health_check();
    let app = build_app_with(proxy, policy, audit).merge(readiness::router(health));

    Server {
        app,
        outbox: outbox_task,
        checkpoint,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::Notify;

    use super::CheckpointHandle;

    #[tokio::test]
    async fn stopping_a_checkpoint_handle_signals_and_joins_the_task() {
        // A stand-in scheduler task that exits once its shutdown is signaled,
        // exactly as `CheckpointScheduler::run` does at its loop boundary.
        let shutdown = Arc::new(Notify::new());
        let task = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { shutdown.notified().await }
        });
        let handle = CheckpointHandle { task, shutdown };

        // Returns only once the task observed the signal and exited — a hang
        // here would fail the test, proving the cooperative stop joins cleanly.
        handle.stop().await;
    }
}
