use std::convert::Infallible;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use froodi::Inject;
use futures::StreamExt;
use uuid::Uuid;

use super::inspect_stream;
use crate::application::common::ports::{RunRequest, UpstreamAgentClient};
use crate::application::inspection::{InspectionContext, Inspector};
use crate::domain::inspection::{Budgets, RunId, SessionId};
use crate::infrastructure::ReqwestAgentClient;

/// Hop-by-hop / framing headers the proxy must not forward verbatim.
const SKIP_HEADERS: [&str; 4] = ["host", "content-length", "connection", "transfer-encoding"];

pub fn router() -> Router {
    Router::new()
        .route("/", post(proxy_run))
        .route("/healthz", get(healthz))
}

async fn healthz() -> &'static str {
    "ok"
}

/// Reverse-proxy a run: forward the client's `RunAgentInput` to the agent, then
/// stream the agent's SSE response back through inspection.
async fn proxy_run(
    Inject(inspector): Inject<Inspector>,
    Inject(client): Inject<ReqwestAgentClient>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request = RunRequest {
        body,
        headers: forwardable_headers(&headers),
    };

    let upstream = match client.run(request).await {
        Ok(stream) => stream,
        Err(error) => return (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
    };

    let context = InspectionContext::new(SessionId(Uuid::new_v4()), RunId(Uuid::new_v4()));
    let inspected = inspect_stream(upstream, inspector, context, Budgets::default())
        .map(Ok::<Bytes, Infallible>);

    Response::builder()
        .header(CONTENT_TYPE, "text/event-stream")
        .body(Body::from_stream(inspected))
        .expect("a streaming body is a valid response")
}

fn forwardable_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter(|(name, _)| !SKIP_HEADERS.contains(&name.as_str()))
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_owned(), value.to_owned()))
        })
        .collect()
}
