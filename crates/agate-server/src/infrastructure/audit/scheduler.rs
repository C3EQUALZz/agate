use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info, warn};

use agate_audit::domain::merkle::{LogId, TreeSize};

use super::issuer::CheckpointIssuer;
use super::scope::ScopeError;

/// Periodically issues a signed checkpoint (STH) for one log — the log's own
/// tamper-evidence cadence, the way a transparency log publishes tree heads on a
/// schedule rather than only when asked.
///
/// It tracks the last issued tree size and passes it down, so an idle log
/// between ticks is signed-and-returned but not re-recorded or re-anchored. A
/// failed issue is logged and the loop continues — a stalled checkpoint must
/// never take down the proxy.
pub struct CheckpointScheduler {
    issuer: Arc<dyn CheckpointIssuer>,
    log: LogId,
    period: Duration,
}

impl CheckpointScheduler {
    #[must_use]
    pub fn new(issuer: Arc<dyn CheckpointIssuer>, log: LogId, period: Duration) -> Self {
        Self {
            issuer,
            log,
            period,
        }
    }

    /// Run until `shutdown` is signaled: issue immediately, then once per
    /// `period`. Ticks missed while an issue runs long are coalesced (we only
    /// need the latest).
    ///
    /// Shutdown is cooperative and checked only at the loop boundary, so the
    /// scheduler never stops in the middle of an issue — it never abandons a
    /// half-open audit scope/transaction the way an abrupt `abort()` could.
    pub async fn run(self, shutdown: Arc<Notify>) {
        let mut tick = interval(self.period);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        debug!(log = %self.log.0, period_secs = self.period.as_secs(), "checkpoint scheduler started");

        let mut last_size: Option<TreeSize> = None;
        loop {
            tokio::select! {
                // Bias the shutdown branch so a pending stop wins over a ready
                // tick, ending promptly without one more issue.
                biased;
                () = shutdown.notified() => {
                    debug!(log = %self.log.0, "checkpoint scheduler stopping");
                    return;
                }
                _ = tick.tick() => {}
            }
            match self.issuer.issue(self.log, last_size).await {
                Ok(sth) => {
                    let size = sth.head.size;
                    if Some(size) == last_size {
                        debug!(log = %self.log.0, size = size.value(), "checkpoint unchanged; not re-anchored");
                    } else {
                        info!(log = %self.log.0, size = size.value(), "issued signed checkpoint");
                    }
                    last_size = Some(size);
                }
                Err(ScopeError::Unavailable(error)) => {
                    warn!(log = %self.log.0, %error, "checkpoint scheduler: cannot open request scope");
                }
                Err(ScopeError::Pipeline(error)) => {
                    error!(log = %self.log.0, ?error, "checkpoint scheduler: issue failed");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    use async_trait::async_trait;
    use tokio::sync::Notify;
    use uuid::Uuid;

    use agate_audit::domain::merkle::{LogId, SignedTreeHead, TreeSize};

    use super::super::issuer::CheckpointIssuer;
    use super::super::scope::ScopeError;
    use super::CheckpointScheduler;

    /// Records the `previous_size` of each issue and signals once, so the test
    /// can wait for the first (immediate) tick deterministically.
    struct FakeIssuer {
        calls: Mutex<Vec<Option<TreeSize>>>,
        issued: Notify,
    }

    #[async_trait]
    impl CheckpointIssuer for FakeIssuer {
        async fn issue(
            &self,
            _log: LogId,
            previous_size: Option<TreeSize>,
        ) -> Result<SignedTreeHead, ScopeError> {
            self.calls.lock().unwrap().push(previous_size);
            self.issued.notify_one();
            // The scope is genuinely unavailable in this unit test (no
            // container) — exercise the error path the scheduler tolerates.
            Err(ScopeError::Unavailable("no container under test".into()))
        }
    }

    #[tokio::test]
    async fn issues_on_the_first_tick_then_stops_cleanly_on_signal() {
        let issuer = Arc::new(FakeIssuer {
            calls: Mutex::new(Vec::new()),
            issued: Notify::new(),
        });
        // A long period: only the immediate first tick fires before we stop.
        let scheduler =
            CheckpointScheduler::new(issuer.clone(), LogId(Uuid::nil()), Duration::from_hours(1));
        let shutdown = Arc::new(Notify::new());
        let handle = tokio::spawn(scheduler.run(shutdown.clone()));

        issuer.issued.notified().await;
        // Cooperative stop: the loop returns at its next boundary, so the task
        // joins cleanly rather than being aborted mid-flight.
        shutdown.notify_one();
        handle.await.expect("scheduler stops cleanly on signal");

        let calls = issuer.calls.lock().unwrap();
        assert_eq!(
            *calls,
            vec![None],
            "first tick issues with no previous size"
        );
    }
}
