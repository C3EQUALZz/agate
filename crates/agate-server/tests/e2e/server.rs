//! Boot the proxy + audit and assert a proxied run lands in the transparency log.

use agate_audit::domain::merkle::LeafIndex;
use agate_policy::domain::decision::PolicyRuleset;

use crate::fixture::{self, spawn};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proxied_run_is_recorded_to_the_transparency_log() {
    let app = spawn(PolicyRuleset::allow_all(), fixture::SSE_BODY).await;

    let body = fixture::client()
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

    let container = fixture::audit_container(app.pool.clone());
    let registry = fixture::audit_registry();
    let recorded = fixture::poll_inclusion(&container, &registry, app.log, LeafIndex(2)).await;
    assert!(
        recorded,
        "the three inspected events were recorded to the log"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn readyz_reports_ready_when_the_database_is_reachable() {
    let app = spawn(PolicyRuleset::allow_all(), fixture::SSE_BODY).await;

    let response = fixture::client()
        .get(format!("{}/readyz", app.base_url))
        .send()
        .await
        .expect("readiness probe responds");

    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.expect("read body"), "ready");
}
