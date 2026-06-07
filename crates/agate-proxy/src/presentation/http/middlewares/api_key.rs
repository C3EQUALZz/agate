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

/// Require a valid `X-API-Key` on `router` when any non-blank key is configured.
///
/// A request is accepted if its key matches **any** of `keys` — so several keys
/// are valid at once, which is how rotation works (add the new key, migrate
/// clients, then drop the old). An empty set (after trimming blanks) leaves the
/// route open and the router unchanged.
pub fn apply(router: Router, keys: &[String]) -> Router {
    let accepted: Vec<String> = keys
        .iter()
        .map(|key| key.trim().to_owned())
        .filter(|key| !key.is_empty())
        .collect();
    if accepted.is_empty() {
        return router;
    }
    let accepted: Arc<[String]> = Arc::from(accepted);
    router.layer(middleware::from_fn_with_state(accepted, require_api_key))
}

/// Reject requests whose `X-API-Key` header matches none of the accepted keys.
async fn require_api_key(
    State(accepted): State<Arc<[String]>>,
    request: Request,
    next: Next,
) -> Response {
    let provided = request
        .headers()
        .get(API_KEY_HEADER)
        .and_then(|value| value.to_str().ok());

    if provided.is_some_and(|key| matches_any(key.as_bytes(), &accepted)) {
        next.run(request).await
    } else {
        warn!("rejected a request with a missing or invalid API key");
        (StatusCode::UNAUTHORIZED, "missing or invalid API key").into_response()
    }
}

/// Whether `provided` equals any accepted key. Folds over **all** keys (no
/// early exit) so a match doesn't leak which key matched through timing; each
/// comparison is itself constant-time for equal-length keys.
fn matches_any(provided: &[u8], accepted: &[String]) -> bool {
    accepted.iter().fold(false, |matched, key| {
        matched | constant_time_eq(provided, key.as_bytes())
    })
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
    use super::{constant_time_eq, matches_any};

    #[test]
    fn constant_time_eq_matches_equal_keys_only() {
        assert!(constant_time_eq(b"s3cret", b"s3cret"));
        assert!(!constant_time_eq(b"s3cret", b"s3creT"));
        assert!(!constant_time_eq(b"s3cret", b"s3cret-longer"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn matches_any_accepts_any_configured_key() {
        let keys = vec!["old-key".to_owned(), "new-key".to_owned()];
        // Either valid key authenticates (the rotation overlap window).
        assert!(matches_any(b"old-key", &keys));
        assert!(matches_any(b"new-key", &keys));
        // A non-member does not.
        assert!(!matches_any(b"other", &keys));
        // No keys configured → nothing matches (caller treats this as "open"
        // and never installs the middleware).
        assert!(!matches_any(b"anything", &[]));
    }
}
