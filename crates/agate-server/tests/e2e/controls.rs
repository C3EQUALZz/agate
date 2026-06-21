//! Drive the argument-deny, result-deny, and state-secret-deny controls through
//! the full booted proxy → audit path. Each is proven in the policy domain by
//! unit tests, but had no end-to-end coverage: this asserts the verdict reaches
//! the wire (the offending frame never leaves) and the event is recorded.

use agate_audit::domain::merkle::LeafIndex;
use agate_policy::domain::decision::{
    ArgumentRule, Pattern, PolicyRuleset, ResultRule, ToolPolicy,
};

use crate::fixture::{self, spawn};

/// Drive `sse` behind `ruleset`, returning the booted server and the streamed
/// client body.
async fn drive(ruleset: PolicyRuleset, sse: &'static str) -> (fixture::TestServer, String) {
    let app = spawn(ruleset, sse).await;
    let body = fixture::client()
        .post(&app.base_url)
        .body("{}")
        .send()
        .await
        .expect("proxy responds")
        .text()
        .await
        .expect("read streamed body");
    (app, body)
}

/// Assert the offending event was *denied*, not merely masked: the run's
/// lifecycle is forwarded, neither `marker` (the forbidden content) nor
/// `frame_type` (the AG-UI event type, so the whole frame was dropped — not
/// forwarded with masked content) reaches the client, and the denial is recorded
/// at leaf `index`.
async fn assert_frame_denied(
    app: &fixture::TestServer,
    body: &str,
    marker: &str,
    frame_type: &str,
    index: u64,
) {
    assert!(
        body.contains("RUN_STARTED") && body.contains("RUN_FINISHED"),
        "lifecycle forwarded: {body}"
    );
    assert!(
        !body.contains(marker),
        "denied content `{marker}` leaked: {body}"
    );
    assert!(
        !body.contains(frame_type),
        "denied `{frame_type}` frame leaked: {body}"
    );
    let container = fixture::audit_container(app.pool.clone());
    let registry = fixture::audit_registry();
    assert!(
        fixture::poll_inclusion(&container, &registry, app.log, LeafIndex(index)).await,
        "the denial was recorded (start + deny + finish)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn argument_rule_denies_a_tool_call_through_the_proxy() {
    // Tool names are unrestricted, but a permitted call whose arguments contain
    // the marker `danger` is denied — the response-leg argument-deny path.
    let ruleset = PolicyRuleset::new(
        ToolPolicy::AllowAll,
        vec![ArgumentRule::new(
            None,
            Pattern::literal("danger").expect("pattern"),
        )],
        vec![],
    );
    let sse = concat!(
        "data: {\"type\":\"RUN_STARTED\"}\n\n",
        "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\",\"toolCallName\":\"run\"}\n\n",
        "data: {\"type\":\"TOOL_CALL_ARGS\",\"toolCallId\":\"c1\",\"delta\":\"{\\\"cmd\\\":\\\"danger\\\"}\"}\n\n",
        "data: {\"type\":\"TOOL_CALL_END\",\"toolCallId\":\"c1\"}\n\n",
        "data: {\"type\":\"RUN_FINISHED\"}\n\n",
    );
    let (app, body) = drive(ruleset, sse).await;
    assert_frame_denied(&app, &body, "danger", "TOOL_CALL", 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn result_rule_denies_a_tool_result_through_the_proxy() {
    // A tool result whose content carries the forbidden marker is dropped before
    // the client — the response-leg result-deny path.
    let ruleset = PolicyRuleset::new(ToolPolicy::AllowAll, vec![], vec![]).with_result_rules(vec![
        ResultRule::new(None, Pattern::literal("TOPSECRET").expect("pattern")),
    ]);
    let sse = concat!(
        "data: {\"type\":\"RUN_STARTED\"}\n\n",
        "data: {\"type\":\"TOOL_CALL_RESULT\",\"toolCallId\":\"c1\",\"content\":\"leaked TOPSECRET value\"}\n\n",
        "data: {\"type\":\"RUN_FINISHED\"}\n\n",
    );
    let (app, body) = drive(ruleset, sse).await;
    assert_frame_denied(&app, &body, "TOPSECRET", "TOOL_CALL_RESULT", 2).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_secret_in_a_state_snapshot_is_denied_through_the_proxy() {
    // A secret in a state payload cannot be masked in place, so it is denied
    // (not redacted) — proven here end-to-end, not just in the domain.
    let ruleset = PolicyRuleset::new(
        ToolPolicy::AllowAll,
        vec![],
        vec![Pattern::literal("sk-LEAK").expect("pattern")],
    );
    let sse = concat!(
        "data: {\"type\":\"RUN_STARTED\"}\n\n",
        "data: {\"type\":\"STATE_SNAPSHOT\",\"snapshot\":{\"token\":\"sk-LEAK\"}}\n\n",
        "data: {\"type\":\"RUN_FINISHED\"}\n\n",
    );
    let (app, body) = drive(ruleset, sse).await;
    assert_frame_denied(&app, &body, "sk-LEAK", "STATE_SNAPSHOT", 2).await;
}
