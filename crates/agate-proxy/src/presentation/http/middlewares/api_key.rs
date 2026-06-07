//! API-key authentication middleware for the proxied route.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use tracing::warn;

/// The header carrying the API key when authentication is enabled.
const API_KEY_HEADER: &str = "x-api-key";

/// Require a matching `X-API-Key` on `router` when `key` is `Some`; a `None`
/// (or blank) key leaves the route open and the router unchanged.
pub fn apply(router: Router, key: Option<&str>) -> Router {
    match key {
        Some(key) => {
            let expected: Arc<str> = Arc::from(key);
            router.layer(middleware::from_fn_with_state(expected, require_api_key))
        }
        None => router,
    }
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
