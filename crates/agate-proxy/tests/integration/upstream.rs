//! The reqwest upstream client against a real stub AG-UI agent: it forwards the
//! run and streams the SSE response back.

use bytes::Bytes;
use futures::StreamExt;

use agate_proxy::application::common::ports::{RunRequest, UpstreamAgentClient};
use agate_proxy::infrastructure::ReqwestAgentClient;

use crate::fixture::{SSE_BODY, stub_agent};

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
async fn unreachable_agent_is_an_upstream_error() {
    let client = ReqwestAgentClient::new("http://127.0.0.1:1/run".to_string());

    let result = client
        .run(RunRequest {
            body: Bytes::from_static(b"{}"),
            headers: vec![],
        })
        .await;

    assert!(result.is_err());
}
