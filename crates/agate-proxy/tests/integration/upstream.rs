//! The reqwest upstream client against a real stub AG-UI agent: it forwards the
//! run and streams the SSE response back.

use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;

use agate_proxy::application::common::ports::{RunRequest, UpstreamAgentClient, UpstreamError};
use agate_proxy::infrastructure::ReqwestAgentClient;

use crate::fixture::{SSE_BODY, stub_agent, stub_failing_agent, stub_stalling_agent};

fn request() -> RunRequest {
    RunRequest {
        body: Bytes::from_static(b"{}"),
        headers: vec![],
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forwards_the_run_and_streams_the_response() {
    let client = ReqwestAgentClient::new(stub_agent().await);

    let mut stream = client
        .run(RunRequest {
            body: Bytes::from_static(b"{\"threadId\":\"t\"}"),
            headers: vec![("content-type".to_string(), "application/json".to_string())],
        })
        .await
        .expect("run");

    let mut collected = Vec::new();
    while let Some(chunk) = stream.next().await {
        collected.extend_from_slice(&chunk.expect("chunk"));
    }

    assert_eq!(String::from_utf8(collected).unwrap(), SSE_BODY);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unreachable_agent_is_a_connect_error() {
    let client = ReqwestAgentClient::new("http://127.0.0.1:1/run".to_string());

    let error = client.run(request()).await.err().expect("the run fails");

    assert!(
        matches!(error, UpstreamError::Connect(_)),
        "a refused port classifies as Connect: {error:?}",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_agent_5xx_is_a_status_error() {
    let client = ReqwestAgentClient::new(stub_failing_agent().await);

    let error = client.run(request()).await.err().expect("the run fails");

    assert_eq!(
        error,
        UpstreamError::Status(500),
        "a 500 answer classifies as Status(500)",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stalled_response_body_is_a_timeout() {
    let http = reqwest::Client::builder()
        .read_timeout(Duration::from_millis(100))
        .build()
        .expect("build the test client");
    let client = ReqwestAgentClient::with_client(http, stub_stalling_agent().await);

    let mut stream = client.run(request()).await.expect("headers arrive");
    let chunk = stream.next().await.expect("the read deadline fires");

    assert_eq!(chunk, Err(UpstreamError::Timeout));
}
