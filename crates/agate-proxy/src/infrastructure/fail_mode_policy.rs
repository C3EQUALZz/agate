//! A [`PolicyPort`] decorator that bounds the wrapped policy's decision time and
//! applies a configured **fail mode** when it is exceeded.
//!
//! The real policy may consult external services and could hang; this guards the
//! data plane against that without the policy implementations knowing about
//! timeouts (composition over modification — the Decorator pattern). The secure
//! default is fail-closed.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::time::timeout;
use tracing::warn;

use crate::application::common::ports::PolicyPort;
use crate::application::inspection::InspectionContext;
use crate::domain::inspection::{AgentEvent, DenyReason, Verdict};

/// What to do when a policy decision cannot be obtained within the deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailMode {
    /// Forward the event (availability over safety).
    Open,
    /// Terminate the run (safety over availability) — the secure default.
    Closed,
}

/// Wraps a [`PolicyPort`], bounding each decision by `timeout` and applying
/// `mode` when the wrapped policy does not answer in time.
pub struct FailModePolicy {
    inner: Arc<dyn PolicyPort>,
    mode: FailMode,
    timeout: Duration,
}

impl FailModePolicy {
    #[must_use]
    pub fn new(inner: Arc<dyn PolicyPort>, mode: FailMode, timeout: Duration) -> Self {
        Self {
            inner,
            mode,
            timeout,
        }
    }
}

#[async_trait]
impl PolicyPort for FailModePolicy {
    async fn decide(&self, context: &InspectionContext, event: &AgentEvent) -> Verdict<AgentEvent> {
        match timeout(self.timeout, self.inner.decide(context, event)).await {
            Ok(verdict) => verdict,
            Err(_elapsed) => {
                warn!(
                    run = %context.run.0,
                    mode = ?self.mode,
                    "policy decision timed out; applying the fail mode",
                );
                match self.mode {
                    FailMode::Open => Verdict::Allow,
                    FailMode::Closed => Verdict::Terminate(DenyReason::new(
                        "policy decision timed out (fail-closed)",
                    )),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::inspection::{LifecyclePhase, RunId, SessionId};

    /// A policy that never answers within a test's lifetime (far longer than any
    /// test timeout, so the timeout always fires first and drops this future).
    struct HangingPolicy;
    #[async_trait]
    impl PolicyPort for HangingPolicy {
        async fn decide(&self, _: &InspectionContext, _: &AgentEvent) -> Verdict<AgentEvent> {
            tokio::time::sleep(Duration::from_secs(30)).await;
            Verdict::Allow
        }
    }

    /// A policy that answers instantly.
    struct InstantDeny;
    #[async_trait]
    impl PolicyPort for InstantDeny {
        async fn decide(&self, _: &InspectionContext, _: &AgentEvent) -> Verdict<AgentEvent> {
            Verdict::Deny(DenyReason::new("nope"))
        }
    }

    fn context() -> InspectionContext {
        InspectionContext::new(SessionId(Uuid::nil()), RunId(Uuid::nil()))
    }

    fn event() -> AgentEvent {
        AgentEvent::Lifecycle(LifecyclePhase::RunStarted)
    }

    #[tokio::test]
    async fn fail_closed_terminates_on_timeout() {
        let policy = FailModePolicy::new(
            Arc::new(HangingPolicy),
            FailMode::Closed,
            Duration::from_millis(10),
        );
        assert!(matches!(
            policy.decide(&context(), &event()).await,
            Verdict::Terminate(_),
        ));
    }

    #[tokio::test]
    async fn fail_open_allows_on_timeout() {
        let policy = FailModePolicy::new(
            Arc::new(HangingPolicy),
            FailMode::Open,
            Duration::from_millis(10),
        );
        assert_eq!(policy.decide(&context(), &event()).await, Verdict::Allow);
    }

    #[tokio::test]
    async fn a_fast_policy_passes_through_unchanged() {
        let policy = FailModePolicy::new(
            Arc::new(InstantDeny),
            FailMode::Closed,
            Duration::from_secs(5),
        );
        assert!(matches!(
            policy.decide(&context(), &event()).await,
            Verdict::Deny(_),
        ));
    }
}
