//! Shared integration fixture: a stub AG-UI agent that answers `POST /run` with
//! a fixed SSE stream, booted on an ephemeral port.

#![allow(dead_code)]

use axum::Router;
use axum::http::header::CONTENT_TYPE;
use axum::routing::post;
use tokio::net::TcpListener;

pub const SSE_BODY: &str =
    "data: {\"type\":\"RUN_STARTED\"}\n\ndata: {\"type\":\"RUN_FINISHED\"}\n\n";

/// Boot the stub agent and return its `/run` URL. The server runs for the test's
/// lifetime on a background task.
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
