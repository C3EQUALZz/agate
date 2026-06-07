use std::convert::Infallible;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use froodi::Inject;
use futures::StreamExt;
use tracing::{info, warn};
use uuid::Uuid;

use super::inspect_stream;
use crate::application::common::ports::{ProxyMetrics, RunRequest, UpstreamAgentClient};
use crate::application::inspection::{InspectionContext, Inspector, RequestDecision};
use crate::domain::inspection::{Budgets, RunId, SessionId};
use crate::infrastructure::ag_ui::parse_request;
use crate::infrastructure::{ProxyMetricsRecorder, ReqwestAgentClient};
use crate::setup::configs::ProxyConfig;

/// Hop-by-hop / framing headers the proxy must not forward verbatim.
const SKIP_HEADERS: [&str; 4] = ["host", "content-length", "connection", "transfer-encoding"];

/// The header carrying the API key when authentication is enabled.
const API_KEY_HEADER: &str = "x-api-key";

/// Build the proxy router, applying the ingress guards from `config`:
/// a request body-size limit and (optionally) an API-key check on the proxied
/// route. The `/healthz` liveness probe is added *after* the layers, so probes
/// are never body-limited or required to authenticate.
pub fn router(config: &ProxyConfig) -> Router {
    let mut run = Router::new()
        .route("/", post(proxy_run))
        .layer(DefaultBodyLimit::max(config.max_body_bytes));

    if let Some(key) = &config.api_key {
        let expected: Arc<str> = Arc::from(key.as_str());
        run = run.layer(middleware::from_fn_with_state(expected, require_api_key));
    }

    run.route("/healthz", get(healthz))
}

async fn healthz() -> &'static str {
    "ok"
}

/// Reject requests whose `X-API-Key` header is missing or does not match the
/// configured key. The comparison is constant-time to avoid leaking the key
/// through response timing.
async fn require_api_key(
    State(expected): State<Arc<str>>,
    request: Request,
    next: Next,
) -> Response {
    let provided = request
        .headers()
        .get(API_KEY_HEADER)
        .and_then(|value| value.to_str().ok());

    match provided {
        Some(key) if constant_time_eq(key.as_bytes(), expected.as_bytes()) => {
            next.run(request).await
        }
        _ => {
            warn!("rejected a request with a missing or invalid API key");
            (StatusCode::UNAUTHORIZED, "missing or invalid API key").into_response()
        }
    }
}

/// Length-checked, branch-free byte comparison (the length itself is not secret).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0_u8;
    for (left, right) in a.iter().zip(b) {
        diff |= left ^ right;
    }
    diff == 0
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
) -> Response {
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
    // SSRF URLs without forwarding.
    let inbound = match parse_request(&body) {
        Ok(inbound) => inbound,
        Err(error) => {
            warn!(run = %context.run.0, %error, "rejecting a malformed request body");
            return (StatusCode::BAD_REQUEST, format!("invalid request: {error}")).into_response();
        }
    };
    if let RequestDecision::Reject(reason) = inspector.inspect_request(&context, &inbound).await {
        info!(run = %context.run.0, reason = reason.as_str(), "request denied on the request leg");
        return (StatusCode::FORBIDDEN, reason.as_str().to_owned()).into_response();
    }

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

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn constant_time_eq_matches_equal_keys_only() {
        assert!(constant_time_eq(b"s3cret", b"s3cret"));
        assert!(!constant_time_eq(b"s3cret", b"s3creT"));
        assert!(!constant_time_eq(b"s3cret", b"s3cret-longer"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }
}
