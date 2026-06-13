//! HTTP middlewares layered onto the proxied route by
//! [`router`](super::router): API-key authentication, a concurrency cap, and a
//! per-client-IP request-rate limit.

/// API-key authentication on the `X-API-Key` header.
pub mod api_key;
/// In-flight concurrency cap that sheds over-capacity requests.
pub mod concurrency;
/// Per-client-IP request-rate limit that sheds floods with `429`.
pub mod rate_limit;
