//! The one place that maps a proxied run's request-path failures to HTTP
//! responses — which status code each failure returns lives here, not scattered
//! across the handler.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// A failure handling a proxied run *before* the response stream begins. Each
/// variant carries an operator-facing message and maps to a fixed HTTP status.
#[derive(Debug)]
pub enum ProxyError {
    /// The request body was not a valid `RunAgentInput` → `400 Bad Request`.
    MalformedRequest(String),
    /// Policy denied the request on the request leg → `403 Forbidden`.
    Denied(String),
    /// The upstream agent could not be reached or failed → `502 Bad Gateway`.
    Upstream(String),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            Self::MalformedRequest(error) => {
                (StatusCode::BAD_REQUEST, format!("invalid request: {error}"))
            }
            Self::Denied(reason) => (StatusCode::FORBIDDEN, reason),
            Self::Upstream(error) => (StatusCode::BAD_GATEWAY, error),
        };
        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::ProxyError;
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
            ProxyError::Upstream("down".to_owned())
                .into_response()
                .status(),
            StatusCode::BAD_GATEWAY,
        );
    }
}
