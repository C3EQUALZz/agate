//! Assert the policy adapter enforces redaction and tool denial through the
//! full proxy → audit path, observable in the client's stream.

use std::collections::BTreeSet;

use agate_audit::domain::merkle::LeafIndex;
use agate_policy::domain::decision::{PolicyRuleset, SecretPattern, ToolName, ToolPolicy};

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

fn ruleset() -> PolicyRuleset {
    // Permit only "search" (so "rm_rf" is denied) and redact the literal "sk-LEAK".
    let allowlist: BTreeSet<ToolName> = [ToolName::new("search").expect("valid tool")]
        .into_iter()
        .collect();
    PolicyRuleset::new(
        ToolPolicy::Allowlist(allowlist),
        vec![SecretPattern::new("sk-LEAK").expect("valid pattern")],
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn policy_redacts_secrets_and_denies_unlisted_tools() {
    let app = spawn(ruleset(), SSE).await;

    let body = reqwest::Client::new()
        .post(&app.base_url)
        .body("{}")
        .send()
        .await
        .expect("proxy responds")
        .text()
        .await
        .expect("read streamed body");

    // Lifecycle still flows through.
    assert!(
        body.contains("RUN_STARTED") && body.contains("RUN_FINISHED"),
        "lifecycle forwarded: {body}"
    );
    // The secret is redacted: the original is gone, the mask is present.
    assert!(!body.contains("sk-LEAK"), "secret leaked to client: {body}");
    assert!(body.contains("[REDACTED]"), "expected a redaction: {body}");
    // The unlisted tool call is denied and dropped entirely.
    assert!(!body.contains("rm_rf"), "denied tool leaked: {body}");
    assert!(
        !body.contains("TOOL_CALL"),
        "denied tool frames leaked: {body}"
    );

    // Every Ready event is recorded regardless of verdict (lifecycle ×2, the
    // redacted message, and the denied tool call) — leaves 0..=3.
    let container = fixture::audit_container(app.pool.clone());
    let registry = fixture::audit_registry();
    let present = fixture::poll_inclusion(&container, &registry, app.log, LeafIndex(3)).await;
    assert!(
        present,
        "expected four records including the deny and redact"
    );
}
