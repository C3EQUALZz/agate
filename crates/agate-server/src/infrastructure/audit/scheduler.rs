use std::sync::Arc;
use std::time::Duration;

use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info, warn};

use agate_audit::domain::merkle::{LogId, TreeSize};

use super::issuer::{CheckpointIssuer, IssueError};

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

    /// Run until aborted: issue immediately, then once per `period`. Ticks
    /// missed while an issue runs long are coalesced (we only need the latest).
    pub async fn run(self) {
        let mut tick = interval(self.period);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        debug!(log = %self.log.0, period_secs = self.period.as_secs(), "checkpoint scheduler started");

        let mut last_size: Option<TreeSize> = None;
        loop {
            tick.tick().await;
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
                Err(IssueError::ScopeUnavailable(error)) => {
                    warn!(log = %self.log.0, %error, "checkpoint scheduler: cannot open request scope");
                }
                Err(IssueError::Pipeline(error)) => {
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

    use super::super::issuer::{CheckpointIssuer, IssueError};
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
        ) -> Result<SignedTreeHead, IssueError> {
            self.calls.lock().unwrap().push(previous_size);
            self.issued.notify_one();
            // The scope is genuinely unavailable in this unit test (no
            // container) — exercise the error path the scheduler tolerates.
            Err(IssueError::ScopeUnavailable(
                "no container under test".into(),
            ))
        }
    }

    #[tokio::test]
    async fn issues_on_the_first_tick_with_no_previous_size() {
        let issuer = Arc::new(FakeIssuer {
            calls: Mutex::new(Vec::new()),
            issued: Notify::new(),
        });
        // A long period: only the immediate first tick fires before we abort.
        let scheduler =
            CheckpointScheduler::new(issuer.clone(), LogId(Uuid::nil()), Duration::from_hours(1));
        let handle = tokio::spawn(scheduler.run());

        issuer.issued.notified().await;
        handle.abort();

        let calls = issuer.calls.lock().unwrap();
        assert_eq!(
            *calls,
            vec![None],
            "first tick issues with no previous size"
        );
    }
}
