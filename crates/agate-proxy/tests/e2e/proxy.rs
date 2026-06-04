//! Full HTTP end-to-end: a client run goes through the booted proxy to a stub
//! agent and the inspected SSE response comes back.

use axum::http::header::CONTENT_TYPE;

use crate::fixture::{spawn_proxy, stub_agent};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proxies_and_inspects_a_run() {
    let agent = stub_agent().await;
    let proxy = spawn_proxy(agent).await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{proxy}/"))
        .body("{\"threadId\":\"t\",\"runId\":\"r\"}")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.starts_with("text/event-stream"));

    // Allow-all policy: the agent's events come through to the client.
    let body = response.text().await.unwrap();
    assert!(body.contains("RUN_STARTED"));
    assert!(body.contains("RUN_FINISHED"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn healthz_is_ok() {
    let proxy = spawn_proxy(stub_agent().await).await;

    let response = reqwest::get(format!("{proxy}/healthz")).await.unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "ok");
}
