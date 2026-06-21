//! Lifecycle e2e: a policy hot-reload is visible to a *booted, serving* proxy.
//!
//! The CEL adapter's reload is unit-tested in isolation (`cel_adapter.rs`), and
//! the SIGHUP / file-watch triggers that call it are tested in `cel_adapter` and
//! `cel_watch`. What none of those cover is the wiring end-to-end: that after a
//! reload the **running** proxy — request → inspect → `PolicyPort` → SSE back to
//! the client — applies the new rules on the very next request. That is the gap
//! this test closes, driving the reload through the same `Arc<CelPolicyAdapter>`
//! the booted server holds (an in-place `ArcSwap`, shared across the await
//! boundary of a live request).

use std::sync::Arc;

use agate_proxy::application::common::ports::PolicyPort;
use agate_server::infrastructure::policy::CelPolicyAdapter;

use crate::fixture::{client, spawn_with_policy};

/// One run that calls the `delete_file` tool, wrapped in a lifecycle. Whether the
/// tool's frames reach the client is decided entirely by the live policy.
const TOOL_CALL_SSE: &str = concat!(
    "data: {\"type\":\"RUN_STARTED\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\",\"toolCallName\":\"delete_file\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_ARGS\",\"toolCallId\":\"c1\",\"delta\":\"{}\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_END\",\"toolCallId\":\"c1\"}\n\n",
    "data: {\"type\":\"RUN_FINISHED\"}\n\n",
);

/// Write `source` to a temp file and load a CEL engine from it. The file is
/// returned so the test can rewrite it and keeps it alive (RAII) for the run.
fn loaded(source: &str) -> (tempfile::NamedTempFile, Arc<CelPolicyAdapter>) {
    let file = tempfile::NamedTempFile::new().expect("temp file");
    std::fs::write(file.path(), source).expect("write policy");
    let adapter = CelPolicyAdapter::load(file.path().to_str().expect("utf-8 path"), 1000)
        .expect("CEL compiles");
    (file, Arc::new(adapter))
}

async fn run_body(base_url: &str) -> String {
    client()
        .post(base_url)
        .body("{}")
        .send()
        .await
        .expect("proxy responds")
        .text()
        .await
        .expect("read streamed body")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reloaded_policy_is_applied_by_a_booted_proxy() {
    // v1 denies an unrelated tool, so `delete_file` is allowed and its frames
    // flow through to the client.
    let (file, adapter) = loaded(
        r#"
        [[rule]]
        when = 'action.name == "format_disk"'
        effect = "deny"
    "#,
    );

    let app = spawn_with_policy(adapter.clone() as Arc<dyn PolicyPort>, TOOL_CALL_SSE).await;

    let before = run_body(&app.base_url).await;
    assert!(
        before.contains("delete_file") && before.contains("TOOL_CALL"),
        "v1 allows the tool, so its frames reach the client: {before}"
    );

    // Swap the file's policy to deny `delete_file` and reload in place — the
    // booted proxy holds the same adapter, so no restart is needed.
    std::fs::write(
        file.path(),
        r#"
        [[rule]]
        when = 'action.name == "delete_file"'
        effect = "deny"
    "#,
    )
    .expect("rewrite policy");
    assert_eq!(adapter.reload().expect("reload succeeds"), 1);

    // The very next request through the same serving proxy applies v2.
    let after = run_body(&app.base_url).await;
    assert!(
        before.contains("RUN_STARTED") && after.contains("RUN_STARTED"),
        "the run still streams either way: {after}"
    );
    assert!(
        !after.contains("delete_file"),
        "v2 denies the tool — its name must not leak after reload: {after}"
    );
    assert!(
        !after.contains("TOOL_CALL"),
        "v2 denies the tool — no tool frames after reload: {after}"
    );
}
