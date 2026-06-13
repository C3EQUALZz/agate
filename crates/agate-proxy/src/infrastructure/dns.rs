use std::net::IpAddr;
use std::time::Duration;

use async_trait::async_trait;
use tokio::net::lookup_host;
use tokio::time::timeout;
use tracing::debug;

use crate::application::common::ports::HostResolver;

/// How long to wait for a hostname to resolve before giving up. Short: the
/// resolve sits on the request hot path, and a host that resolves slowly is one
/// the agent would struggle to reach anyway. A timeout yields no addresses
/// (fail-open — the literal checks still apply).
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(2);

/// The system-resolver [`HostResolver`] (Tokio's `lookup_host`). Resolution
/// failure or timeout yields an empty result, never an error: the SSRF guard
/// treats "could not resolve" as non-blocking.
pub struct TokioHostResolver;

#[async_trait]
impl HostResolver for TokioHostResolver {
    async fn resolve(&self, host: &str) -> Vec<IpAddr> {
        // `lookup_host` wants a host:port; the port is irrelevant to the address.
        match timeout(RESOLVE_TIMEOUT, lookup_host((host, 0))).await {
            Ok(Ok(addrs)) => addrs.map(|addr| addr.ip()).collect(),
            Ok(Err(error)) => {
                debug!(host, %error, "host did not resolve; SSRF guard falls back to literal checks");
                Vec::new()
            }
            Err(_) => {
                debug!(
                    host,
                    "host resolution timed out; SSRF guard falls back to literal checks"
                );
                Vec::new()
            }
        }
    }
}

/// A resolver that resolves nothing — for the permissive default app and tests
/// that don't exercise DNS rebinding.
pub struct NoopHostResolver;

#[async_trait]
impl HostResolver for NoopHostResolver {
    async fn resolve(&self, _host: &str) -> Vec<IpAddr> {
        Vec::new()
    }
}
