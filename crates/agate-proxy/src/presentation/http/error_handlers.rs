//! The one place that maps a proxied run's request-path failures to HTTP
//! responses — which status code each failure returns lives here, not scattered
//! across the handler.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::application::common::ports::UpstreamError;

/// A failure handling a proxied run *before* the response stream begins. Each
/// variant carries an operator-facing message and maps to a fixed HTTP status.
#[derive(Debug)]
pub enum ProxyError {
    /// The request body was not a valid `RunAgentInput` → `400 Bad Request`.
    MalformedRequest(String),
    /// Policy denied the request on the request leg → `403 Forbidden`.
    Denied(String),
    /// The upstream agent failed: a timeout → `504 Gateway Timeout`, anything
    /// else → `502 Bad Gateway`.
    Upstream(UpstreamError),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        // The client body is a stable, sanitized message — the detailed upstream
        // error (which can carry the agent's host/topology) stays in the logs,
        // never in the response, so the proxy does not become an SSRF oracle.
        let (status, body) = match self {
            Self::MalformedRequest(error) => {
                (StatusCode::BAD_REQUEST, format!("invalid request: {error}"))
            }
            Self::Denied(reason) => (StatusCode::FORBIDDEN, reason),
            Self::Upstream(UpstreamError::Timeout) => (
                StatusCode::GATEWAY_TIMEOUT,
                "upstream agent timed out".into(),
            ),
            Self::Upstream(_) => (
                StatusCode::BAD_GATEWAY,
                "upstream agent request failed".into(),
            ),
        };
        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::{ProxyError, UpstreamError};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    #[test]
    fn maps_each_error_to_its_status() {
        assert_eq!(
            ProxyError::MalformedRequest("bad".to_owned())
                .into_response()
                .status(),
            StatusCode::BAD_REQUEST,
        );
        assert_eq!(
            ProxyError::Denied("denied".to_owned())
                .into_response()
                .status(),
            StatusCode::FORBIDDEN,
        );
        assert_eq!(
            ProxyError::Upstream(UpstreamError::Connect("down".to_owned()))
                .into_response()
                .status(),
            StatusCode::BAD_GATEWAY,
        );
        assert_eq!(
            ProxyError::Upstream(UpstreamError::Status(500))
                .into_response()
                .status(),
            StatusCode::BAD_GATEWAY,
        );
    }

    #[test]
    fn an_upstream_timeout_is_a_gateway_timeout() {
        assert_eq!(
            ProxyError::Upstream(UpstreamError::Timeout)
                .into_response()
                .status(),
            StatusCode::GATEWAY_TIMEOUT,
        );
    }
}
