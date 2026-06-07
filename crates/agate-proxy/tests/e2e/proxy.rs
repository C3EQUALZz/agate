//! Full HTTP end-to-end: a client run goes through the booted proxy to a stub
//! agent and the inspected SSE response comes back.

use std::time::Duration;

use axum::http::header::CONTENT_TYPE;

use agate_proxy::setup::configs::ProxyConfig;

use crate::fixture::{spawn_proxy, spawn_proxy_config, stub_agent};

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

/// Builds a config for `agent` with an API key and a 16-byte body limit.
fn hardened(agent: String, api_key: Option<&str>, max_body_bytes: usize) -> ProxyConfig {
    ProxyConfig::new(agent, "127.0.0.1:0".to_string()).with_ingress(
        Duration::from_secs(5),
        Duration::from_mins(1),
        max_body_bytes,
        api_key.map(str::to_owned).into_iter().collect(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_key_gate_rejects_unauthenticated_runs_but_lets_probes_through() {
    let agent = stub_agent().await;
    let proxy = spawn_proxy_config(hardened(agent, Some("s3cret"), 1 << 20)).await;
    let client = reqwest::Client::new();
    let run_body = "{\"threadId\":\"t\",\"runId\":\"r\"}";

    // No key → 401, before the run ever reaches the agent.
    let missing = client
        .post(format!("{proxy}/"))
        .body(run_body)
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 401);

    // Wrong key → 401.
    let wrong = client
        .post(format!("{proxy}/"))
        .header("X-API-Key", "nope")
        .body(run_body)
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 401);

    // Correct key → forwarded (200).
    let ok = client
        .post(format!("{proxy}/"))
        .header("X-API-Key", "s3cret")
        .body(run_body)
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);

    // The liveness probe bypasses authentication.
    let probe = reqwest::get(format!("{proxy}/healthz")).await.unwrap();
    assert_eq!(probe.status(), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn body_size_limit_rejects_oversized_requests() {
    let agent = stub_agent().await;
    let proxy = spawn_proxy_config(hardened(agent, None, 16)).await;

    let response = reqwest::Client::new()
        .post(format!("{proxy}/"))
        .body("x".repeat(1024))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 413);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_request_body_is_rejected_before_forwarding() {
    let proxy = spawn_proxy(stub_agent().await).await;

    let response = reqwest::Client::new()
        .post(format!("{proxy}/"))
        .body("this is not json")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_with_an_ssrf_url_is_denied() {
    let proxy = spawn_proxy(stub_agent().await).await;

    // A user message embedding a loopback URL — denied by the request-leg SSRF
    // guard before the agent ever runs, even under the default allow-all policy.
    let body = r#"{"threadId":"t","runId":"r","messages":[{"id":"m","role":"user","content":"fetch http://127.0.0.1/secret"}]}"#;
    let response = reqwest::Client::new()
        .post(format!("{proxy}/"))
        .body(body)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 403);
}
