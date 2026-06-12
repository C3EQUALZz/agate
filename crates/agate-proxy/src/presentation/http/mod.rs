use std::convert::Infallible;

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
use crate::application::common::ports::RunRequest;
use crate::application::inspection::{InspectionContext, Inspector, RequestDecision};
use crate::domain::inspection::{RunId, SessionId};
use crate::infrastructure::ag_ui::parse_request;
use crate::setup::configs::ProxyConfig;
use crate::setup::ioc::{ProxyMetricsHandle, UpstreamAgentClientHandle};

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

    run = middlewares::api_key::apply(run, &config.api_keys);
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
    Inject(client): Inject<UpstreamAgentClientHandle>,
    Inject(metrics): Inject<ProxyMetricsHandle>,
    Inject(config): Inject<ProxyConfig>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let client = client.0.clone();
    let metrics = metrics.0.clone();
    let context =
        InspectionContext::new(SessionId::new(Uuid::new_v4()), RunId::new(Uuid::new_v4()));
    info!(
        session = %context.session,
        run = %context.run,
        "run received; forwarding to upstream agent"
    );
    metrics.record_run();

    // Request leg (preventive): validate the body and inspect it before the
    // agent ever runs — reject malformed input, denied tools, secret markers, or
    // SSRF URLs without forwarding. The status for each failure is decided in
    // `error_handlers`; here we only attach context and log.
    let inbound = parse_request(&body).map_err(|error| {
        warn!(run = %context.run, %error, "rejecting a malformed request body");
        ProxyError::MalformedRequest(error.to_string())
    })?;
    if let RequestDecision::Reject(reason) = inspector.inspect_request(&context, &inbound).await {
        info!(run = %context.run, reason = reason.as_str(), "request denied on the request leg");
        return Err(ProxyError::Denied(reason.as_str().to_owned()));
    }

    let request = RunRequest {
        body,
        headers: forwardable_headers(&headers),
    };

    let upstream = client.run(request).await.map_err(|error| {
        warn!(run = %context.run, %error, "upstream agent request failed");
        metrics.record_upstream_error(&error);
        ProxyError::Upstream(error)
    })?;

    let inspected = inspect_stream(
        upstream,
        inspector,
        context,
        config.inspection_settings(),
        metrics,
    )
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
