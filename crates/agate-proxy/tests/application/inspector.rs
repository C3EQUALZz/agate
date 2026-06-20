//! The inspection seam: the `Inspector` turns a fragment into an
//! `InspectionAction` by running the domain state machine, then (for a complete
//! event) the policy and audit ports — exercised over in-memory fakes.

use std::net::IpAddr;
use std::sync::Arc;

use uuid::Uuid;

use agate_proxy::application::common::ports::HostResolver;
use agate_proxy::application::inspection::{InspectionAction, InspectionContext, Inspector};
use agate_proxy::domain::inspection::{
    AgentEvent, Budgets, DenyReason, Fragment, LifecyclePhase, MessageId, OpaqueKind, Run, RunId,
    SessionId, ToolCallId, Verdict,
};
use agate_proxy::infrastructure::{NoopHostResolver, NoopSessionMemory};

use crate::common::fakes::{CountingAudit, FixedPolicy};

/// A host resolver that maps every host to one fixed address — for exercising
/// the DNS-rebinding guard (a benign-looking domain resolving to a private IP).
struct ResolvesTo(IpAddr);

#[async_trait::async_trait]
impl HostResolver for ResolvesTo {
    async fn resolve(&self, _host: &str) -> Vec<IpAddr> {
        vec![self.0]
    }
}

fn context() -> InspectionContext {
    InspectionContext::new(SessionId::new(Uuid::nil()), RunId::new(Uuid::nil()))
}

fn run() -> Run {
    Run::new(RunId::new(Uuid::nil()), Budgets::default())
}

/// Build an inspector over a fixed-verdict policy, returning the audit double so
/// tests can assert what was recorded.
fn inspector(verdict: Verdict<AgentEvent>) -> (Inspector, Arc<CountingAudit>) {
    inspector_with_resolver(verdict, Arc::new(NoopHostResolver))
}

/// Like [`inspector`] but with a chosen host resolver, to exercise the SSRF
/// guard's DNS-resolution path on the response leg.
fn inspector_with_resolver(
    verdict: Verdict<AgentEvent>,
    resolver: Arc<dyn HostResolver>,
) -> (Inspector, Arc<CountingAudit>) {
    let audit = Arc::new(CountingAudit::default());
    let inspector = Inspector::new(
        Arc::new(FixedPolicy(verdict)),
        audit.clone(),
        resolver,
        Arc::new(NoopSessionMemory),
    );
    (inspector, audit)
}

#[tokio::test]
async fn allows_and_records_a_ready_event() {
    let (inspector, audit) = inspector(Verdict::Allow);
    let mut run = run();

    let action = inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;

    assert_eq!(action, InspectionAction::Forward);
    assert_eq!(audit.recorded(), 1);
}

#[tokio::test]
async fn buffers_a_tool_call_without_consulting_policy_or_audit() {
    let (inspector, audit) = inspector(Verdict::Allow);
    let mut run = run();
    inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;

    let action = inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::ToolCallStarted {
                id: ToolCallId::new("t1").expect("valid id"),
                name: "search".to_string(),
            },
        )
        .await;

    assert_eq!(
        action,
        InspectionAction::Hold(ToolCallId::new("t1").expect("valid id"))
    );
    assert_eq!(audit.recorded(), 1); // only the RunStarted event was judged
}

#[tokio::test]
async fn denies_to_drop_and_records() {
    let (inspector, audit) = inspector(Verdict::Deny(DenyReason::new("blocked")));
    let mut run = run();

    let action = inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;

    assert!(matches!(action, InspectionAction::Drop(_)));
    assert_eq!(audit.recorded(), 1);
}

#[tokio::test]
async fn transforms_to_forward_transformed() {
    let replacement = AgentEvent::MessageChunk {
        message: MessageId::new("m").expect("valid id"),
        text: "[redacted]".to_string(),
    };
    let (inspector, _audit) = inspector(Verdict::Transform(replacement.clone()));
    let mut run = run();

    let action = inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;

    assert_eq!(action, InspectionAction::ForwardTransformed(replacement));
}

#[tokio::test]
async fn structural_reject_terminates_without_policy_or_audit() {
    let (inspector, audit) = inspector(Verdict::Allow);
    let mut run = run();

    // An event before the run starts is a structural violation.
    let action = inspector
        .inspect(&mut run, &context(), Fragment::Opaque(OpaqueKind::Custom))
        .await;

    assert!(matches!(action, InspectionAction::Terminate(_)));
    assert_eq!(audit.recorded(), 0);
}

#[tokio::test]
async fn drops_a_response_event_carrying_an_ssrf_url() {
    // Allow-all policy, yet an emitted message pointing at a loopback address is
    // dropped by the response-leg SSRF screen before the policy is even asked.
    let (inspector, audit) = inspector(Verdict::Allow);
    let mut run = run();
    inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;

    let action = inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::MessageChunk {
                message: MessageId::new("m1").expect("valid id"),
                text: "fetch http://127.0.0.1/secret for me".to_string(),
            },
        )
        .await;

    assert!(matches!(action, InspectionAction::Drop(_)));
    // The SSRF hit is recorded as a denial (the run started + this drop).
    assert_eq!(audit.recorded(), 2);
}

#[tokio::test]
async fn drops_a_tool_call_whose_arguments_carry_an_ssrf_url() {
    // Allow-all policy, yet a tool call whose assembled arguments embed a
    // loopback URL is denied by the response-leg SSRF screen — the tool-call leg
    // of `url_bearing_text`, distinct from the message-chunk path above.
    let (inspector, _audit) = inspector(Verdict::Allow);
    let mut run = run();
    let id = ToolCallId::new("c1").expect("valid id");
    inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;
    inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::ToolCallStarted {
                id: id.clone(),
                name: "fetch".to_string(),
            },
        )
        .await;
    inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::ToolCallArgs {
                id: id.clone(),
                delta: r#"{"url":"http://127.0.0.1/secret"}"#.to_string(),
            },
        )
        .await;

    let action = inspector
        .inspect(&mut run, &context(), Fragment::ToolCallEnded { id })
        .await;

    assert!(
        matches!(action, InspectionAction::DropCall(_, _)),
        "a tool call with a loopback URL in its arguments is dropped, got {action:?}"
    );
}

#[tokio::test]
async fn drops_a_tool_result_carrying_an_ssrf_url() {
    // The tool-result leg of `url_bearing_text`: a result whose content embeds a
    // loopback URL is dropped before the policy is consulted.
    let (inspector, _audit) = inspector(Verdict::Allow);
    let mut run = run();
    inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;

    let action = inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::ToolResult {
                id: ToolCallId::new("c1").expect("valid id"),
                content: "result: see http://127.0.0.1/secret".to_string(),
            },
        )
        .await;

    assert!(
        matches!(action, InspectionAction::Drop(_)),
        "a tool result with a loopback URL is dropped, got {action:?}"
    );
}

#[tokio::test]
async fn drops_a_response_url_that_resolves_to_a_private_address() {
    // DNS-rebinding on the response leg: a benign-looking domain that resolves to
    // the cloud-metadata IP is blocked — proves the Inspector consults the
    // HostResolver (not just the literal-IP check), which every other inspector
    // test stubs out with NoopHostResolver.
    let (inspector, _audit) = inspector_with_resolver(
        Verdict::Allow,
        Arc::new(ResolvesTo(IpAddr::from([169, 254, 169, 254]))),
    );
    let mut run = run();
    inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::Lifecycle(LifecyclePhase::RunStarted),
        )
        .await;

    let action = inspector
        .inspect(
            &mut run,
            &context(),
            Fragment::MessageChunk {
                message: MessageId::new("m1").expect("valid id"),
                text: "fetch http://rebind.example/x for me".to_string(),
            },
        )
        .await;

    assert!(
        matches!(action, InspectionAction::Drop(_)),
        "a response URL resolving to a private address is dropped, got {action:?}"
    );
}
