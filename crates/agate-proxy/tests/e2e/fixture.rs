//! End-to-end fixture: a stub AG-UI agent and the proxy booted in front of it,
//! both on ephemeral ports.

#![allow(dead_code)]

use axum::Router;
use axum::http::header::CONTENT_TYPE;
use axum::routing::post;
use tokio::net::TcpListener;

use agate_proxy::setup::bootstrap::build_app;
use agate_proxy::setup::configs::ProxyConfig;

pub const SSE_BODY: &str =
    "data: {\"type\":\"RUN_STARTED\"}\n\ndata: {\"type\":\"RUN_FINISHED\"}\n\n";

/// Boot a stub agent answering `POST /run` with a fixed SSE stream; return its
/// `/run` URL.
pub async fn stub_agent() -> String {
    let app = Router::new().route(
        "/run",
        post(|| async { ([(CONTENT_TYPE, "text/event-stream")], SSE_BODY) }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}/run")
}

/// Boot the proxy in front of `agent_endpoint`; return its base URL.
pub async fn spawn_proxy(agent_endpoint: String) -> String {
    let app = build_app(ProxyConfig::new(agent_endpoint, "127.0.0.1:0".to_string()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}
