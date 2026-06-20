//! Exercise the CEL and Rego plugin policy backends through the full booted
//! proxy → audit path — not just their in-crate unit tests. Each engine, loaded
//! from a real policy file and wired into `build_server` as the `PolicyPort`,
//! must deny a tool and redact a secret in the client stream, and record every
//! event to the transparency log.

use std::sync::Arc;

use agate_proxy::application::common::ports::PolicyPort;
use agate_server::infrastructure::policy::{CelPolicyAdapter, RegoPolicyAdapter};

use crate::fixture::{self, spawn_with_policy};

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

/// Boot the proxy behind `policy` and assert the shared redact-and-deny outcome
/// — the same check the static-ruleset e2e makes, now for a plugin engine.
async fn assert_denies_and_redacts(policy: Arc<dyn PolicyPort>) {
    let app = spawn_with_policy(policy, fixture::REDACT_DENY_SSE).await;
    fixture::assert_redacts_secret_and_denies_tool(&app).await;
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
