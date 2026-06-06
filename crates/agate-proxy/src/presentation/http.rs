use std::convert::Infallible;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use froodi::Inject;
use futures::StreamExt;
use tracing::{info, warn};
use uuid::Uuid;

use super::inspect_stream;
use crate::application::common::ports::{ProxyMetrics, RunRequest, UpstreamAgentClient};
use crate::application::inspection::{InspectionContext, Inspector};
use crate::domain::inspection::{Budgets, RunId, SessionId};
use crate::infrastructure::{ProxyMetricsRecorder, ReqwestAgentClient};

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
    Inject(metrics): Inject<ProxyMetricsRecorder>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let metrics: Arc<dyn ProxyMetrics> = metrics;
    let context = InspectionContext::new(SessionId(Uuid::new_v4()), RunId(Uuid::new_v4()));
    info!(
        session = %context.session.0,
        run = %context.run.0,
        "run received; forwarding to upstream agent"
    );
    metrics.record_run();

    let request = RunRequest {
        body,
        headers: forwardable_headers(&headers),
    };

    let upstream = match client.run(request).await {
        Ok(stream) => stream,
        Err(error) => {
            warn!(run = %context.run.0, %error, "upstream agent request failed");
            metrics.record_upstream_error();
            return (StatusCode::BAD_GATEWAY, error.to_string()).into_response();
        }
    };

    let inspected = inspect_stream(upstream, inspector, context, Budgets::default(), metrics)
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
