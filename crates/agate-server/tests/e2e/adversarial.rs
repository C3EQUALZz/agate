//! A capstone "kitchen-sink" of attack vectors driven through the booted proxy
//! under an **allow-all** policy, so each is stopped by a *built-in* screen — not
//! a configured rule (those are covered by `controls.rs`). Response-leg SSRF in a
//! tool call's arguments, a message, and a tool result; and a malformed known
//! event terminating the run.

use agate_policy::domain::decision::{PolicyRuleset, ToolPolicy};

use crate::fixture::{self, spawn};

/// No tool restriction, no argument/result rules, no redaction markers — every
/// block below is a built-in screen, not an operator rule.
fn allow_all() -> PolicyRuleset {
    PolicyRuleset::new(ToolPolicy::AllowAll, vec![], vec![])
}

/// Boot under the allow-all policy, drive `sse`, return the streamed client body.
async fn body_for(sse: &'static str) -> String {
    let app = spawn(allow_all(), sse).await;
    fixture::client()
        .post(&app.base_url)
        .body("{}")
        .send()
        .await
        .expect("proxy responds")
        .text()
        .await
        .expect("read streamed body")
}

/// The run streamed (start + finish) but none of `forbidden` reached the client.
fn assert_forwarded_without(body: &str, forbidden: &[&str]) {
    assert!(
        body.contains("RUN_STARTED") && body.contains("RUN_FINISHED"),
        "lifecycle forwarded: {body}"
    );
    for needle in forbidden {
        assert!(
            !body.contains(needle),
            "`{needle}` leaked to client: {body}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn response_leg_ssrf_in_tool_arguments_is_dropped() {
    // A tool call whose JSON arguments embed a loopback URL is dropped by the
    // built-in SSRF screen (the case the #105 fix closed: URLs inside JSON).
    let body = body_for(concat!(
        "data: {\"type\":\"RUN_STARTED\"}\n\n",
        "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\",\"toolCallName\":\"fetch\"}\n\n",
        "data: {\"type\":\"TOOL_CALL_ARGS\",\"toolCallId\":\"c1\",\"delta\":\"{\\\"url\\\":\\\"http://169.254.169.254/latest\\\"}\"}\n\n",
        "data: {\"type\":\"TOOL_CALL_END\",\"toolCallId\":\"c1\"}\n\n",
        "data: {\"type\":\"RUN_FINISHED\"}\n\n",
    ))
    .await;
    assert_forwarded_without(&body, &["169.254.169.254", "TOOL_CALL"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn response_leg_ssrf_in_a_message_is_dropped() {
    let body = body_for(concat!(
        "data: {\"type\":\"RUN_STARTED\"}\n\n",
        "data: {\"type\":\"TEXT_MESSAGE_CONTENT\",\"messageId\":\"m1\",\"delta\":\"fetch http://127.0.0.1/secret\"}\n\n",
        "data: {\"type\":\"RUN_FINISHED\"}\n\n",
    ))
    .await;
    assert_forwarded_without(&body, &["127.0.0.1"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn response_leg_ssrf_in_a_tool_result_is_dropped() {
    let body = body_for(concat!(
        "data: {\"type\":\"RUN_STARTED\"}\n\n",
        "data: {\"type\":\"TOOL_CALL_RESULT\",\"toolCallId\":\"c1\",\"content\":\"see http://10.0.0.5/x\"}\n\n",
        "data: {\"type\":\"RUN_FINISHED\"}\n\n",
    ))
    .await;
    assert_forwarded_without(&body, &["10.0.0.5", "TOOL_CALL_RESULT"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_malformed_known_event_terminates_the_run() {
    // A recognized event missing a required field (`messageId`) is uninspectable;
    // the default `on_malformed_event = terminate` ends the run with RUN_ERROR
    // rather than forwarding it unchecked — so the run never reaches RUN_FINISHED.
    let body = body_for(concat!(
        "data: {\"type\":\"RUN_STARTED\"}\n\n",
        "data: {\"type\":\"TEXT_MESSAGE_CONTENT\",\"delta\":\"no message id\"}\n\n",
        "data: {\"type\":\"RUN_FINISHED\"}\n\n",
    ))
    .await;
    assert!(body.contains("RUN_STARTED"), "run started: {body}");
    assert!(
        body.contains("RUN_ERROR"),
        "the malformed event terminated the run with RUN_ERROR: {body}"
    );
    assert!(
        !body.contains("RUN_FINISHED"),
        "a terminated run must not reach RUN_FINISHED: {body}"
    );
}
