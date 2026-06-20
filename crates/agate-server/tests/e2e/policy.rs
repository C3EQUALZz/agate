//! Assert the policy adapter enforces redaction and tool denial through the
//! full proxy → audit path, observable in the client's stream.

use agate_policy::domain::decision::{Pattern, PolicyRuleset, ToolMatcher, ToolPolicy};

use crate::fixture::{self, spawn};

/// Permit only `search` (so `rm_rf` is denied) and redact the literal `sk-LEAK`.
fn ruleset() -> PolicyRuleset {
    let allowlist = vec![ToolMatcher::exact("search").expect("valid tool")];
    PolicyRuleset::new(
        ToolPolicy::Allowlist(allowlist),
        vec![],
        vec![Pattern::literal("sk-LEAK").expect("valid pattern")],
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn policy_redacts_secrets_and_denies_unlisted_tools() {
    let app = spawn(ruleset(), fixture::REDACT_DENY_SSE).await;
    fixture::assert_redacts_secret_and_denies_tool(&app).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_offering_a_denied_tool_is_rejected_before_forwarding() {
    let app = spawn(ruleset(), fixture::REDACT_DENY_SSE).await;

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
