//! Request-leg inspection: the preventive checks applied to an incoming
//! `RunAgentInput` *before* it is forwarded to the agent.
//!
//! Structural parsing lives in the AG-UI adapter; this module holds the
//! security facts ([`RequestContent`]), the decision type ([`RequestDecision`]),
//! and the URL / SSRF guard. Tool-authorization and secret-marker decisions
//! reuse the same [`PolicyPort`](crate::application::common::ports::PolicyPort)
//! as the response leg — the [`Inspector`](super::Inspector) projects each
//! offered tool and user message onto an `AgentEvent` and asks the policy.

use std::net::{Ipv4Addr, Ipv6Addr};

use url::{Host, Url};

use crate::domain::inspection::DenyReason;

/// The security-relevant facts parsed from a `RunAgentInput` for the request leg.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestContent {
    /// Names of the tools the client offers the agent.
    pub offered_tools: Vec<String>,
    /// Text of the user messages.
    pub user_messages: Vec<String>,
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
/// A best-effort SSRF guard: it classifies literal hosts only and does **not**
/// resolve DNS, so DNS-rebinding is out of scope.
#[must_use]
pub fn first_disallowed_url(text: &str) -> Option<DenyReason> {
    text.split_whitespace()
        .filter(|token| token.contains("://"))
        .find_map(classify_url)
        .map(DenyReason::new)
}

fn classify_url(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(['.', ',', '(', ')', '[', ']', '{', '}', '"', '\'', '<', '>']);
    let url = Url::parse(trimmed).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return Some(format!("disallowed URL scheme `{}`", url.scheme()));
    }
    match url.host()? {
        Host::Domain(domain) => {
            let domain = domain.to_ascii_lowercase();
            (domain == "localhost"
                || domain.ends_with(".localhost")
                || domain == "metadata.google.internal")
                .then(|| format!("URL host `{domain}` is not allowed"))
        }
        Host::Ipv4(ip) => {
            is_blocked_v4(ip).then(|| format!("URL host {ip} is a private/loopback address"))
        }
        Host::Ipv6(ip) => {
            is_blocked_v6(ip).then(|| format!("URL host [{ip}] is a private/loopback address"))
        }
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
    use super::first_disallowed_url;

    #[test]
    fn allows_public_http_urls_and_plain_text() {
        assert!(first_disallowed_url("see https://example.com/docs for details").is_none());
        assert!(first_disallowed_url("no urls here at all").is_none());
        assert!(first_disallowed_url("http://93.184.216.34/page").is_none());
    }

    #[test]
    fn blocks_loopback_private_and_metadata_hosts() {
        assert!(first_disallowed_url("fetch http://localhost:8080/x").is_some());
        assert!(first_disallowed_url("fetch http://127.0.0.1/x").is_some());
        assert!(first_disallowed_url("grab http://10.0.0.5/secret").is_some());
        assert!(first_disallowed_url("http://169.254.169.254/latest/meta-data").is_some());
        assert!(first_disallowed_url("http://[::1]/x").is_some());
        // IPv4-mapped IPv6 must not bypass the IPv4 rules.
        assert!(first_disallowed_url("http://[::ffff:127.0.0.1]/x").is_some());
        assert!(first_disallowed_url("http://[::ffff:10.0.0.5]/x").is_some());
    }

    #[test]
    fn blocks_non_http_schemes() {
        assert!(first_disallowed_url("file:///etc/passwd").is_some());
        assert!(first_disallowed_url("gopher://evil/x").is_some());
    }

    #[test]
    fn ignores_trailing_punctuation() {
        // A public URL followed by a period is still allowed.
        assert!(first_disallowed_url("read https://example.com.").is_none());
        // A blocked URL with trailing punctuation is still caught.
        assert!(first_disallowed_url("(http://127.0.0.1/x)").is_some());
    }
}
