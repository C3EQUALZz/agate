//! Cross-run session-memory replay guard through the live proxy: a tool denied
//! in one run is quarantined by name for the rest of the session, so the agent
//! cannot retry it with clean arguments in a later run of the same thread. The
//! quarantine LOGIC is unit-tested in `agate-proxy`; this proves the live wiring
//! — config → in-memory backend in the DI container → `threadId`-derived session
//! shared across HTTP runs.

use std::sync::Arc;
use std::time::Duration;

use agate_policy::application::PolicyService;
use agate_policy::domain::decision::{ArgumentRule, Pattern, PolicyRuleset, ToolPolicy};
use agate_proxy::application::common::ports::PolicyPort;
use agate_proxy::setup::configs::{ProxyConfig, SessionMemoryBackend, SessionMemoryConfig};
use agate_server::infrastructure::policy::PolicyAdapter;

use crate::fixture::{self, spawn_core, stub_agent_sequence};

/// Agent response calling tool `quar`, with the `CMD` placeholder standing in
/// for the argument value. `danger` is denied by the argument rule; `safe` would
/// be allowed by it — so only the session quarantine can deny the `safe` retry.
const CALL_TEMPLATE: &str = concat!(
    "data: {\"type\":\"RUN_STARTED\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\",\"toolCallName\":\"quar\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_ARGS\",\"toolCallId\":\"c1\",\"delta\":\"{\\\"cmd\\\":\\\"CMD\\\"}\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_END\",\"toolCallId\":\"c1\"}\n\n",
    "data: {\"type\":\"RUN_FINISHED\"}\n\n",
);

/// A `quar` call whose argument value is `cmd`.
fn quar_call(cmd: &str) -> String {
    CALL_TEMPLATE.replace("CMD", cmd)
}

/// POST one run on `thread`, returning the streamed client body.
async fn run(base_url: &str, thread: &str, run_id: &str) -> String {
    fixture::client()
        .post(base_url)
        .body(format!(r#"{{"threadId":"{thread}","runId":"{run_id}"}}"#))
        .send()
        .await
        .expect("proxy responds")
        .text()
        .await
        .expect("read streamed body")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tool_denied_in_one_run_is_quarantined_for_the_session() {
    // Tool names are unrestricted; only an argument containing `danger` is denied.
    let ruleset = PolicyRuleset::new(
        ToolPolicy::AllowAll,
        vec![ArgumentRule::new(
            None,
            Pattern::literal("danger").expect("pattern"),
        )],
        vec![],
    );
    let policy: Arc<dyn PolicyPort> = Arc::new(PolicyAdapter::new(PolicyService::new(ruleset)));

    // The agent calls `quar` with dangerous args first, then with clean args.
    let endpoint = stub_agent_sequence(vec![
        quar_call("danger"),
        quar_call("safe"),
        quar_call("safe"),
    ])
    .await;
    let proxy = ProxyConfig::new(endpoint, "127.0.0.1:0".to_string()).with_session_memory(Some(
        SessionMemoryConfig {
            backend: SessionMemoryBackend::InMemory,
            ttl: Duration::from_hours(1),
        },
    ));
    let app = spawn_core(policy, proxy).await;

    // Run 1 (thread A): `quar` with dangerous args is denied by the argument
    // rule, which quarantines the tool name for session A.
    let first = run(&app.base_url, "thread-a", "run-1").await;
    assert!(
        !first.contains("TOOL_CALL"),
        "run 1's dangerous call should be denied: {first}"
    );

    // Run 2 (thread A): the same `quar`, now with clean args the rule would
    // allow — but the session quarantine still denies it.
    let second = run(&app.base_url, "thread-a", "run-2").await;
    assert!(
        !second.contains("TOOL_CALL"),
        "run 2's retry of the quarantined tool should be denied by session memory: {second}"
    );

    // Run 3 (thread B): the identical clean call on a *different* thread is
    // allowed — proving run 2's denial came from the per-session quarantine, not
    // from anything intrinsic to the call.
    let third = run(&app.base_url, "thread-b", "run-3").await;
    assert!(
        third.contains("quar"),
        "the same call on a fresh session is allowed and forwarded: {third}"
    );
}
