//! Exercise the CEL and Rego plugin policy backends through the full booted
//! proxy → audit path — not just their in-crate unit tests. Each engine, loaded
//! from a real policy file and wired into `build_server` as the `PolicyPort`,
//! must deny a tool and redact a secret in the client stream, and record every
//! event to the transparency log.

use std::sync::Arc;

use agate_audit::domain::merkle::LeafIndex;
use agate_proxy::application::common::ports::PolicyPort;
use agate_server::infrastructure::policy::{CelPolicyAdapter, RegoPolicyAdapter};

use crate::fixture::{self, spawn_with_policy};

/// Emits a secret in message text, then calls a tool the policy denies.
const SSE: &str = concat!(
    "data: {\"type\":\"RUN_STARTED\"}\n\n",
    "data: {\"type\":\"TEXT_MESSAGE_CONTENT\",\"messageId\":\"m1\",\"delta\":\"token sk-LEAK end\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\",\"toolCallName\":\"rm_rf\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_ARGS\",\"toolCallId\":\"c1\",\"delta\":\"{}\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_END\",\"toolCallId\":\"c1\"}\n\n",
    "data: {\"type\":\"RUN_FINISHED\"}\n\n",
);

/// CEL equivalent of the static e2e policy: deny `rm_rf`, redact `sk-LEAK`.
const CEL_POLICY: &str = r#"
[[rule]]
when = 'action.kind == "tool_call" && action.name == "rm_rf"'
effect = "deny"
[[rule]]
when = 'action.kind == "message" && action.text.contains("sk-LEAK")'
effect = "redact"
replacement = '"[REDACTED]"'
"#;

/// Rego equivalent of the same policy.
const REGO_POLICY: &str = r#"package agate.policy
import rego.v1

decision := {"effect": "deny"} if {
    input.action.kind == "tool_call"
    input.action.name == "rm_rf"
}

decision := {"effect": "redact", "replacement": "[REDACTED]"} if {
    input.action.kind == "message"
    contains(input.action.text, "sk-LEAK")
}
"#;

/// Write a policy to a temp file (kept alive by the caller until the adapter has
/// loaded + compiled it — adapters read the file only at load / reload).
fn write_policy(source: &str) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().expect("temp file");
    std::fs::write(file.path(), source).expect("write policy");
    file
}

/// Boot the proxy behind `policy`, drive the run, and assert the secret is
/// redacted, the tool is denied, and all four events are recorded.
async fn assert_denies_and_redacts(policy: Arc<dyn PolicyPort>) {
    let app = spawn_with_policy(policy, SSE).await;

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
async fn cel_backend_denies_and_redacts_through_the_proxy() {
    let file = write_policy(CEL_POLICY);
    let policy = Arc::new(
        CelPolicyAdapter::load(file.path().to_str().expect("utf-8 path")).expect("CEL compiles"),
    );
    assert_denies_and_redacts(policy).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rego_backend_denies_and_redacts_through_the_proxy() {
    let file = write_policy(REGO_POLICY);
    let policy = Arc::new(
        RegoPolicyAdapter::load(file.path().to_str().expect("utf-8 path")).expect("Rego compiles"),
    );
    assert_denies_and_redacts(policy).await;
}
