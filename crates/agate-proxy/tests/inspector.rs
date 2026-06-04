//! The inspection seam: the `Inspector` turns a fragment into an
//! `InspectionAction` by running the domain state machine, then (for a complete
//! event) the policy and audit ports.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use uuid::Uuid;

use agate_proxy::application::common::ports::{AuditSink, PolicyPort};
use agate_proxy::application::inspection::{InspectionAction, InspectionContext, Inspector};
use agate_proxy::domain::inspection::{
    AgentEvent, Budgets, DenyReason, Fragment, LifecyclePhase, MessageId, OpaqueKind, Run, RunId,
    SessionId, ToolCallId, Verdict,
};

/// Policy that always returns a configured verdict.
struct FixedPolicy(Verdict<AgentEvent>);

#[async_trait]
impl PolicyPort for FixedPolicy {
    async fn decide(&self, _: &InspectionContext, _: &AgentEvent) -> Verdict<AgentEvent> {
        self.0.clone()
    }
}

/// Audit sink that counts how many events it recorded.
#[derive(Default)]
struct CountingAudit {
    records: AtomicUsize,
}

#[async_trait]
impl AuditSink for CountingAudit {
    async fn record(&self, _: &InspectionContext, _: &AgentEvent, _: &Verdict<AgentEvent>) {
        self.records.fetch_add(1, Ordering::SeqCst);
    }
}

fn context() -> InspectionContext {
    InspectionContext::new(SessionId(Uuid::nil()), RunId(Uuid::nil()))
}

fn run() -> Run {
    Run::new(RunId(Uuid::nil()), Budgets::default())
}

fn inspector(verdict: Verdict<AgentEvent>) -> (Inspector, Arc<CountingAudit>) {
    let audit = Arc::new(CountingAudit::default());
    let inspector = Inspector::new(Arc::new(FixedPolicy(verdict)), audit.clone());
    (inspector, audit)
}

#[tokio::test]
async fn allows_and_records_a_ready_event() {
    let (inspector, audit) = inspector(Verdict::Allow);
    let mut run = run();
    let ctx = context();

    let action = inspector
        .inspect(
            &mut run,
            &ctx,
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;

    assert_eq!(action, InspectionAction::Forward);
    assert_eq!(audit.records.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn buffers_a_tool_call_without_consulting_policy_or_audit() {
    let (inspector, audit) = inspector(Verdict::Allow);
    let mut run = run();
    let ctx = context();
    inspector
        .inspect(
            &mut run,
            &ctx,
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;

    let action = inspector
        .inspect(
            &mut run,
            &ctx,
            Fragment::ToolCallStarted {
                id: ToolCallId("t1".to_string()),
                name: "search".to_string(),
            },
        )
        .await;

    assert_eq!(action, InspectionAction::Hold);
    // Only the RunStarted lifecycle event was judged/recorded.
    assert_eq!(audit.records.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn denies_to_drop_and_records() {
    let (inspector, audit) = inspector(Verdict::Deny(DenyReason::new("blocked")));
    let mut run = run();
    let ctx = context();

    let action = inspector
        .inspect(
            &mut run,
            &ctx,
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;

    assert!(matches!(action, InspectionAction::Drop(_)));
    assert_eq!(audit.records.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn transforms_to_forward_transformed() {
    let replacement = AgentEvent::MessageChunk {
        message: MessageId("m".to_string()),
        text: "[redacted]".to_string(),
    };
    let (inspector, _audit) = inspector(Verdict::Transform(replacement.clone()));
    let mut run = run();
    let ctx = context();

    let action = inspector
        .inspect(
            &mut run,
            &ctx,
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;

    assert_eq!(action, InspectionAction::ForwardTransformed(replacement));
}

#[tokio::test]
async fn structural_reject_terminates_without_policy_or_audit() {
    let (inspector, audit) = inspector(Verdict::Allow);
    let mut run = run();
    let ctx = context();

    // An event before the run starts is a structural violation.
    let action = inspector
        .inspect(&mut run, &ctx, Fragment::Opaque(OpaqueKind::Custom))
        .await;

    assert!(matches!(action, InspectionAction::Terminate(_)));
    assert_eq!(audit.records.load(Ordering::SeqCst), 0);
}
