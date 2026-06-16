use std::net::IpAddr;

use async_trait::async_trait;

/// Resolves a hostname to its IP addresses, so the SSRF guard can re-check the
/// address a URL's host *actually* points at — closing DNS-rebinding, where a
/// public-looking domain resolves to a private/loopback/link-local address.
///
/// An empty result (host unknown, or resolution failed/timed out) is treated as
/// "cannot prove it is dangerous" and does not block on its own: the literal
/// checks still apply, and a host the agent cannot resolve it cannot reach.
#[async_trait]
pub trait HostResolver: Send + Sync {
    /// Resolve `host` to its IP addresses. An empty vector means the host is
    /// unknown or resolution failed/timed out — the SSRF guard treats that as
    /// non-blocking (it never rejects purely because a host did not resolve).
    async fn resolve(&self, host: &str) -> Vec<IpAddr>;
}
