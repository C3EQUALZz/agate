//! Request-leg inspection: the preventive checks applied to an incoming
//! `RunAgentInput` *before* it is forwarded to the agent.
//!
//! Structural parsing lives in the AG-UI adapter; this module holds the
//! security facts ([`RequestContent`]), the decision type ([`RequestDecision`]),
//! and the URL / SSRF guard. Tool-authorization and secret-marker decisions
//! reuse the same [`PolicyPort`](crate::application::common::ports::PolicyPort)
//! as the response leg — the [`Inspector`](super::Inspector) projects each
//! offered tool and user message onto an `AgentEvent` and asks the policy.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use url::{Host, Url};

use crate::application::common::ports::HostResolver;
use crate::domain::inspection::DenyReason;

/// The facts parsed from a `RunAgentInput` for the request leg: the run's
/// identity (so a verdict can be scoped to the conversation) and the
/// security-relevant content to inspect.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestContent {
    /// The AG-UI `threadId` — the conversation this run belongs to, used to
    /// scope per-session state (replay memory). `None` when the client omits it,
    /// in which case the run is treated as its own one-off session.
    pub thread_id: Option<String>,
    /// The AG-UI `runId` — this run's own id, carried into the audit context so
    /// a recorded verdict correlates to the run. `None` when the client omits it.
    pub run_id: Option<String>,
    /// Names of the tools the client offers the agent.
    pub offered_tools: Vec<String>,
    /// Text of the user messages.
    pub user_messages: Vec<String>,
    /// Text of the otherwise-hidden request fields — `system` message content
    /// and the JSON of `context`, `forwardedProps`, and inbound `state` — so an
    /// injection (a secret marker or SSRF URL) hidden there is screened too,
    /// not just `user` messages.
    pub hidden_fields: Vec<String>,
}

/// The outcome of inspecting a request before forwarding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestDecision {
    /// Forward the request to the agent.
    Allow,
    /// Reject the request without forwarding it.
    Reject(DenyReason),
}

/// Find the first URL in `text` the proxy must not let through: a non-`http(s)`
/// scheme, or a host that is loopback / private / link-local (covering the cloud
/// metadata address `169.254.169.254`) or `localhost`.
///
/// Domains that pass the literal checks are **resolved** through `resolver` and
/// re-checked against the same address rules, closing DNS-rebinding (a
/// public-looking domain pointing at a private address). A host that does not
/// resolve is not blocked on that basis — the literal checks still apply.
pub async fn first_disallowed_url(text: &str, resolver: &dyn HostResolver) -> Option<DenyReason> {
    for token in text
        .split_whitespace()
        .filter(|token| token.contains("://"))
    {
        match classify_url(token) {
            UrlVerdict::Allowed => {}
            UrlVerdict::Blocked(reason) => return Some(DenyReason::new(reason)),
            UrlVerdict::Resolve(host) => {
                for ip in resolver.resolve(&host).await {
                    if is_blocked(ip) {
                        return Some(DenyReason::new(format!(
                            "URL host `{host}` resolves to private/loopback address {ip}"
                        )));
                    }
                }
            }
        }
    }
    None
}

/// What a literal URL classification yields before any DNS lookup.
enum UrlVerdict {
    /// Safe by the literal rules (a public IP, or a domain still to resolve).
    Allowed,
    /// Rejected by a literal rule (bad scheme, literal private IP, known host).
    Blocked(String),
    /// A domain to resolve and re-check against the address rules.
    Resolve(String),
}

fn classify_url(token: &str) -> UrlVerdict {
    let trimmed = token.trim_matches(['.', ',', '(', ')', '[', ']', '{', '}', '"', '\'', '<', '>']);
    let Ok(url) = Url::parse(trimmed) else {
        return UrlVerdict::Allowed;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return UrlVerdict::Blocked(format!("disallowed URL scheme `{}`", url.scheme()));
    }
    match url.host() {
        None => UrlVerdict::Allowed,
        Some(Host::Domain(domain)) => {
            let domain = domain.to_ascii_lowercase();
            if domain == "localhost"
                || domain.ends_with(".localhost")
                || domain == "metadata.google.internal"
            {
                UrlVerdict::Blocked(format!("URL host `{domain}` is not allowed"))
            } else {
                UrlVerdict::Resolve(domain)
            }
        }
        Some(Host::Ipv4(ip)) => {
            if is_blocked_v4(ip) {
                UrlVerdict::Blocked(format!("URL host {ip} is a private/loopback address"))
            } else {
                UrlVerdict::Allowed
            }
        }
        Some(Host::Ipv6(ip)) => {
            if is_blocked_v6(ip) {
                UrlVerdict::Blocked(format!("URL host [{ip}] is a private/loopback address"))
            } else {
                UrlVerdict::Allowed
            }
        }
    }
}

fn is_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_blocked_v4(ip),
        IpAddr::V6(ip) => is_blocked_v6(ip),
    }
}

fn is_blocked_v4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
}

fn is_blocked_v6(ip: Ipv6Addr) -> bool {
    // An IPv4-mapped literal (`::ffff:127.0.0.1`) reaches the same host as the
    // bare IPv4, so classify it with the IPv4 rules — otherwise it bypasses them.
    if let Some(mapped) = ip.to_ipv4_mapped() {
        return is_blocked_v4(mapped);
    }
    let first = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || (first & 0xfe00) == 0xfc00 // fc00::/7 unique-local
        || (first & 0xffc0) == 0xfe80 // fe80::/10 link-local
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use async_trait::async_trait;

    use super::first_disallowed_url;
    use crate::application::common::ports::HostResolver;

    /// Resolves nothing — the literal-only behaviour, for the existing cases.
    struct NoResolve;
    #[async_trait]
    impl HostResolver for NoResolve {
        async fn resolve(&self, _host: &str) -> Vec<IpAddr> {
            Vec::new()
        }
    }

    /// Resolves every host to a fixed address — to exercise the rebinding path.
    struct ResolvesTo(IpAddr);
    #[async_trait]
    impl HostResolver for ResolvesTo {
        async fn resolve(&self, _host: &str) -> Vec<IpAddr> {
            vec![self.0]
        }
    }

    async fn literal(text: &str) -> Option<super::DenyReason> {
        first_disallowed_url(text, &NoResolve).await
    }

    #[tokio::test]
    async fn allows_public_http_urls_and_plain_text() {
        assert!(
            literal("see https://example.com/docs for details")
                .await
                .is_none()
        );
        assert!(literal("no urls here at all").await.is_none());
        assert!(literal("http://93.184.216.34/page").await.is_none());
    }

    #[tokio::test]
    async fn blocks_loopback_private_and_metadata_hosts() {
        assert!(literal("fetch http://localhost:8080/x").await.is_some());
        assert!(literal("fetch http://127.0.0.1/x").await.is_some());
        assert!(literal("grab http://10.0.0.5/secret").await.is_some());
        assert!(
            literal("http://169.254.169.254/latest/meta-data")
                .await
                .is_some()
        );
        assert!(literal("http://[::1]/x").await.is_some());
        // IPv4-mapped IPv6 must not bypass the IPv4 rules.
        assert!(literal("http://[::ffff:127.0.0.1]/x").await.is_some());
        assert!(literal("http://[::ffff:10.0.0.5]/x").await.is_some());
    }

    #[tokio::test]
    async fn blocks_non_http_schemes() {
        assert!(literal("file:///etc/passwd").await.is_some());
        assert!(literal("gopher://evil/x").await.is_some());
    }

    #[tokio::test]
    async fn ignores_trailing_punctuation() {
        // A public URL followed by a period is still allowed.
        assert!(literal("read https://example.com.").await.is_none());
        // A blocked URL with trailing punctuation is still caught.
        assert!(literal("(http://127.0.0.1/x)").await.is_some());
    }

    #[tokio::test]
    async fn blocks_a_domain_that_resolves_to_a_private_address() {
        // A public-looking domain that resolves to the cloud metadata IP — the
        // DNS-rebinding case the literal checks miss.
        let resolver = ResolvesTo(IpAddr::from([169, 254, 169, 254]));
        let reason = first_disallowed_url("fetch http://totally-public.example/x", &resolver).await;
        assert!(
            reason.is_some(),
            "rebinding to a private address is blocked"
        );
    }

    #[tokio::test]
    async fn allows_a_domain_that_resolves_to_a_public_address() {
        let resolver = ResolvesTo(IpAddr::from([93, 184, 216, 34]));
        assert!(
            first_disallowed_url("fetch http://example.com/x", &resolver)
                .await
                .is_none()
        );
    }
}
