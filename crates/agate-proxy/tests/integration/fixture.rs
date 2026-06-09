//! Shared integration fixture: a stub AG-UI agent that answers `POST /run` with
//! a fixed SSE stream, booted on an ephemeral port.

#![allow(dead_code)]

use std::convert::Infallible;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::routing::post;
use tokio::net::TcpListener;

pub const SSE_BODY: &str =
    "data: {\"type\":\"RUN_STARTED\"}\n\ndata: {\"type\":\"RUN_FINISHED\"}\n\n";

/// Boot the stub agent and return its `/run` URL. The server runs for the test's
/// lifetime on a background task.
pub async fn stub_agent() -> String {
    serve(Router::new().route(
        "/run",
        post(|| async { ([(CONTENT_TYPE, "text/event-stream")], SSE_BODY) }),
    ))
    .await
}

/// A stub agent that answers every run with `500 Internal Server Error`.
pub async fn stub_failing_agent() -> String {
    serve(Router::new().route(
        "/run",
        post(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "agent exploded") }),
    ))
    .await
}

/// A stub agent that sends the response headers and then stalls forever — the
/// body stream never yields a chunk, so a client read deadline fires.
pub async fn stub_stalling_agent() -> String {
    serve(Router::new().route(
        "/run",
        post(|| async {
            let body = Body::from_stream(futures::stream::pending::<Result<Bytes, Infallible>>());
            ([(CONTENT_TYPE, "text/event-stream")], body)
        }),
    ))
    .await
}

async fn serve(app: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}/run")
}
