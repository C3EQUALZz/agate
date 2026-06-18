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

/// Fixed namespaces so the same AG-UI `threadId` / `runId` always derives the
/// same UUID. That determinism is what lets per-session state recognize a
/// returning conversation across runs (and keeps audit ids correlatable),
/// while two distinct namespaces stop a thread and a run that share a label
/// from colliding.
const SESSION_NAMESPACE: Uuid = Uuid::from_u128(0x8f6b_1d2e_4a7c_4b3e_9f1a_2c5d_7e8f_0a1b);
const RUN_NAMESPACE: Uuid = Uuid::from_u128(0x3c9e_7a1f_6d2b_4e8c_b5a0_1f4d_6e8c_0b2a);

/// Derive a stable `SessionId` UUID from the AG-UI `threadId`, or a fresh random
/// one when it is absent (an unscoped run is its own one-off session).
fn session_uuid(thread_id: Option<&str>) -> Uuid {
    derive_uuid(&SESSION_NAMESPACE, thread_id)
}

/// Derive a stable `RunId` UUID from the AG-UI `runId`, or a fresh random one
/// when it is absent.
fn run_uuid(run_id: Option<&str>) -> Uuid {
    derive_uuid(&RUN_NAMESPACE, run_id)
}

fn derive_uuid(namespace: &Uuid, label: Option<&str>) -> Uuid {
    match label {
        Some(label) => Uuid::new_v5(namespace, label.as_bytes()),
        None => Uuid::new_v4(),
    }
}

/// Build the proxy router, applying the ingress guards from `config`:
/// a request body-size limit, an optional API-key check, a concurrency cap, and
/// a per-client-IP request-rate limit on the proxied route. The `/healthz`
/// liveness probe is added *after* the layers, so probes are never body-limited,
/// authenticated, capped, or rate-limited.
pub fn router(config: &ProxyConfig) -> Router {
    let mut run = Router::new()
        .route("/", post(proxy_run))
        .layer(DefaultBodyLimit::max(config.max_body_bytes));

    run = middlewares::api_key::apply(run, &config.api_keys);
    run = middlewares::concurrency::apply(run, config.max_concurrent_requests);
    // Outermost guard: shed floods by source IP before any per-request work.
    run =
        middlewares::rate_limit::apply(run, config.rate_limit_per_second, config.rate_limit_burst);

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

    // Request leg (preventive): validate the body and inspect it before the
    // agent ever runs — reject malformed input, denied tools, secret markers, or
    // SSRF URLs without forwarding. The status for each failure is decided in
    // `error_handlers`.
    let inbound = parse_request(&body).map_err(|error| {
        warn!(%error, "rejecting a malformed request body");
        ProxyError::MalformedRequest(error.to_string())
    })?;
    // The run's identity comes from the body (`threadId` / `runId`), so the
    // inspection context — and the per-session state keyed on it — is built from
    // the parsed input, not minted blindly. A returning `threadId` therefore
    // maps to the same session, which is what makes replay memory work.
    let context = InspectionContext::new(
        SessionId::new(session_uuid(inbound.thread_id.as_deref())),
        RunId::new(run_uuid(inbound.run_id.as_deref())),
    );
    info!(
        session = %context.session,
        run = %context.run,
        "run received; forwarding to upstream agent"
    );
    metrics.record_run();

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

#[cfg(test)]
mod tests {
    use super::{run_uuid, session_uuid};

    #[test]
    fn the_same_thread_id_derives_the_same_session_uuid() {
        // Stability across runs is what makes per-session replay memory work.
        assert_eq!(
            session_uuid(Some("thread-1")),
            session_uuid(Some("thread-1"))
        );
        assert_ne!(
            session_uuid(Some("thread-1")),
            session_uuid(Some("thread-2"))
        );
    }

    #[test]
    fn an_absent_identifier_is_a_random_one_off_session() {
        // No threadId → a fresh id each time, so an unscoped run shares nothing.
        assert_ne!(session_uuid(None), session_uuid(None));
    }

    #[test]
    fn a_shared_label_does_not_collide_across_the_session_and_run_namespaces() {
        assert_ne!(session_uuid(Some("x")), run_uuid(Some("x")));
    }
}
