//! Boot the proxy + audit and assert a proxied run lands in the transparency log.

use agate_audit::domain::merkle::LeafIndex;
use agate_policy::domain::decision::PolicyRuleset;

use crate::fixture::{self, spawn};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proxied_run_is_recorded_to_the_transparency_log() {
    let app = spawn(PolicyRuleset::allow_all(), fixture::SSE_BODY).await;

    // Drive a run through the proxy: it forwards to the stub agent and streams
    // the inspected SSE back. The body proves the proxy forwarded the run.
    let body = reqwest::Client::new()
        .post(&app.base_url)
        .body("{}")
        .send()
        .await
        .expect("proxy responds")
        .text()
        .await
        .expect("read streamed body");
    assert!(body.contains("RUN_STARTED"), "run forwarded: {body}");
    assert!(body.contains("RUN_FINISHED"), "run finished: {body}");

    // The audit write happens off the hot path via the outbox, so poll until the
    // three events (leaves 0, 1, 2) have been appended.
    let container = fixture::audit_container(app.pool.clone());
    let registry = fixture::audit_registry();
    let present = fixture::poll_inclusion(&container, &registry, app.log, LeafIndex(2)).await;
    assert!(
        present,
        "expected the three inspected events recorded to the log"
    );
}
