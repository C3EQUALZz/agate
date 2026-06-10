//! Assert the policy adapter enforces redaction and tool denial through the
//! full proxy → audit path, observable in the client's stream.

use std::collections::BTreeSet;

use agate_audit::domain::merkle::LeafIndex;
use agate_policy::domain::decision::{Pattern, PolicyRuleset, ToolName, ToolPolicy};

use crate::fixture::{self, spawn};

/// A run that emits a secret in text and then calls a tool not on the allowlist.
const SSE: &str = concat!(
    "data: {\"type\":\"RUN_STARTED\"}\n\n",
    "data: {\"type\":\"TEXT_MESSAGE_CONTENT\",\"messageId\":\"m1\",\"delta\":\"token sk-LEAK end\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\",\"toolCallName\":\"rm_rf\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_ARGS\",\"toolCallId\":\"c1\",\"delta\":\"{}\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_END\",\"toolCallId\":\"c1\"}\n\n",
    "data: {\"type\":\"RUN_FINISHED\"}\n\n",
);

/// Permit only `search` (so `rm_rf` is denied) and redact the literal `sk-LEAK`.
fn ruleset() -> PolicyRuleset {
    let allowlist: BTreeSet<ToolName> = [ToolName::new("search").expect("valid tool")]
        .into_iter()
        .collect();
    PolicyRuleset::new(
        ToolPolicy::Allowlist(allowlist),
        vec![],
        vec![Pattern::literal("sk-LEAK").expect("valid pattern")],
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn policy_redacts_secrets_and_denies_unlisted_tools() {
    let app = spawn(ruleset(), SSE).await;

    let body = fixture::client()
        .post(&app.base_url)
        .body("{}")
        .send()
        .await
        .expect("proxy responds")
        .text()
        .await
        .expect("read streamed body");

    assert!(
        body.contains("RUN_STARTED") && body.contains("RUN_FINISHED"),
        "lifecycle forwarded: {body}"
    );
    assert!(!body.contains("sk-LEAK"), "secret leaked to client: {body}");
    assert!(body.contains("[REDACTED]"), "message was redacted: {body}");
    assert!(!body.contains("rm_rf"), "denied tool leaked: {body}");
    assert!(
        !body.contains("TOOL_CALL"),
        "denied tool frames leaked: {body}"
    );

    let container = fixture::audit_container(app.pool.clone());
    let registry = fixture::audit_registry();
    let recorded = fixture::poll_inclusion(&container, &registry, app.log, LeafIndex(3)).await;
    assert!(
        recorded,
        "all four Ready events recorded (lifecycle x2 + redacted message + denied tool)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_offering_a_denied_tool_is_rejected_before_forwarding() {
    let app = spawn(ruleset(), SSE).await;

    // The ruleset allows only `search`; offering `rm_rf` is denied on the
    // request leg, so the run is rejected (403) and never reaches the agent.
    let response = fixture::client()
        .post(&app.base_url)
        .body(r#"{"threadId":"t","runId":"r","tools":[{"name":"rm_rf"}]}"#)
        .send()
        .await
        .expect("proxy responds");

    assert_eq!(response.status(), 403);
}
