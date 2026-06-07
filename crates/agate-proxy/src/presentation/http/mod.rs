use std::convert::Infallible;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderMap, header::CONTENT_TYPE};
use axum::response::Response;
use axum::routing::{get, post};
use froodi::Inject;
use futures::StreamExt;
use tracing::{info, warn};
use uuid::Uuid;

use self::error_handlers::ProxyError;
use super::inspect_stream;
use crate::application::common::ports::{ProxyMetrics, RunRequest, UpstreamAgentClient};
use crate::application::inspection::{InspectionContext, Inspector, RequestDecision};
use crate::domain::inspection::{Budgets, RunId, SessionId};
use crate::infrastructure::ag_ui::parse_request;
use crate::infrastructure::{ProxyMetricsRecorder, ReqwestAgentClient};
use crate::setup::configs::ProxyConfig;

pub mod error_handlers;
pub mod middlewares;

/// Hop-by-hop / framing headers the proxy must not forward verbatim.
const SKIP_HEADERS: [&str; 4] = ["host", "content-length", "connection", "transfer-encoding"];

/// Build the proxy router, applying the ingress guards from `config`:
/// a request body-size limit, an optional API-key check, and a concurrency cap
/// on the proxied route. The `/healthz` liveness probe is added *after* the
/// layers, so probes are never body-limited, authenticated, or capped.
pub fn router(config: &ProxyConfig) -> Router {
    let mut run = Router::new()
        .route("/", post(proxy_run))
        .layer(DefaultBodyLimit::max(config.max_body_bytes));

    run = middlewares::api_key::apply(run, config.api_key.as_deref());
    // Outermost guard: shed over-capacity requests before any per-request work.
    run = middlewares::concurrency::apply(run, config.max_concurrent_requests);

    run.route("/healthz", get(healthz))
}

async fn healthz() -> &'static str {
    "ok"
}

/// Reverse-proxy a run: forward the client's `RunAgentInput` to the agent, then
/// stream the agent's SSE response back through inspection.
#[tracing::instrument(skip_all)]
async fn proxy_run(
    Inject(inspector): Inject<Inspector>,
    Inject(client): Inject<ReqwestAgentClient>,
    Inject(metrics): Inject<ProxyMetricsRecorder>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let metrics: Arc<dyn ProxyMetrics> = metrics;
    let context = InspectionContext::new(SessionId(Uuid::new_v4()), RunId(Uuid::new_v4()));
    info!(
        session = %context.session.0,
        run = %context.run.0,
        "run received; forwarding to upstream agent"
    );
    metrics.record_run();

    // Request leg (preventive): validate the body and inspect it before the
    // agent ever runs — reject malformed input, denied tools, secret markers, or
    // SSRF URLs without forwarding. The status for each failure is decided in
    // `error_handlers`; here we only attach context and log.
    let inbound = parse_request(&body).map_err(|error| {
        warn!(run = %context.run.0, %error, "rejecting a malformed request body");
        ProxyError::MalformedRequest(error.to_string())
    })?;
    if let RequestDecision::Reject(reason) = inspector.inspect_request(&context, &inbound).await {
        info!(run = %context.run.0, reason = reason.as_str(), "request denied on the request leg");
        return Err(ProxyError::Denied(reason.as_str().to_owned()));
    }

    let request = RunRequest {
        body,
        headers: forwardable_headers(&headers),
    };

    let upstream = client.run(request).await.map_err(|error| {
        warn!(run = %context.run.0, %error, "upstream agent request failed");
        metrics.record_upstream_error();
        ProxyError::Upstream(error.to_string())
    })?;

    let inspected = inspect_stream(upstream, inspector, context, Budgets::default(), metrics)
        .map(Ok::<Bytes, Infallible>);

    Ok(Response::builder()
        .header(CONTENT_TYPE, "text/event-stream")
        .body(Body::from_stream(inspected))
        .expect("a streaming body is a valid response"))
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
