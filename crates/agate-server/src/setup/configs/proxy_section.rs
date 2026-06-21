use serde::{Deserialize, Serialize};

/// `[proxy]` — the reverse-proxy data plane.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxySection {
    /// Upstream agent run endpoint the proxy forwards to (required).
    pub agent_endpoint: String,
    /// Address the proxy listens on.
    pub bind: String,
    /// Connect timeout to the upstream agent, in seconds (fail fast when
    /// unreachable). Not an overall deadline — a healthy SSE stream runs on.
    pub connect_timeout_secs: u64,
    /// Idle read timeout between upstream response chunks, in seconds.
    pub read_timeout_secs: u64,
    /// Maximum accepted request body size, in bytes.
    pub max_body_bytes: usize,
    /// Single API key required on the `X-API-Key` header — a shorthand for one
    /// key. Merged with `api_keys`. Absent/blank (and `api_keys` empty) disables
    /// authentication (open proxy) — set one, or front the proxy with a guard.
    pub api_key: Option<String>,
    /// Accepted API keys: a request matching **any** is authenticated. Holding
    /// several at once is how rotation works (add the new, migrate, drop the old).
    pub api_keys: Vec<String>,
    /// Maximum concurrently in-flight proxied runs; excess is shed with `503`.
    pub max_concurrent_requests: usize,
    /// Per-run ceiling on response events streamed to the client (`0` =
    /// unlimited). A runaway agent over this is cut off with a `RUN_ERROR`. This
    /// is a running counter, not a buffer: `0` removes the client-flood
    /// (availability) guard, not a memory bound — per-run memory stays bounded by
    /// [`max_frame_bytes`](Self::max_frame_bytes) and the tool-call budgets.
    pub max_response_events: usize,
    /// Per-run ceiling on response bytes streamed to the client (`0` =
    /// unlimited). Like [`max_response_events`](Self::max_response_events) this is
    /// a streamed counter, not a buffer the proxy fills: `0` disables the
    /// runaway-output guard (an availability trade-off), it does not let the proxy
    /// grow memory — that is bounded by [`max_frame_bytes`](Self::max_frame_bytes).
    pub max_response_bytes: usize,
    /// Maximum bytes buffered for a single not-yet-complete SSE event. Unlike the
    /// per-run ceilings above, this is charged *while* a frame is still being
    /// received, so an upstream streaming a frame that never terminates cannot
    /// grow the decoder's buffer without bound (the per-run budget only counts
    /// complete events). Crossing it cuts the run off with a `RUN_ERROR`. Must be
    /// greater than 0 — `0` would restore the unbounded behaviour. A well-formed
    /// AG-UI event is a few KiB; the 1 MiB default is generous.
    pub max_frame_bytes: usize,
    /// Sustained per-client-IP request rate, in requests per second
    /// (`0` = disabled, the default). Floods from one source IP over this are
    /// shed with `429`. The IP is the **connection peer**, so enable this only
    /// where Agate sees the real client. Behind a reverse proxy (nginx) or load
    /// balancer every request shares the proxy's IP, so the limit would throttle
    /// *all* clients as one and start rejecting legitimate traffic — leave it `0`
    /// there and rate-limit at the proxy instead.
    pub rate_limit_per_second: u32,
    /// Burst depth for the per-IP rate limit — the largest instantaneous burst
    /// before the sustained rate applies (`0` falls back to
    /// `rate_limit_per_second`).
    pub rate_limit_burst: u32,
}

impl ProxySection {
    /// Fail fast on a missing endpoint or zeroed ingress knobs.
    pub fn validate(&self) -> Result<(), String> {
        if self.agent_endpoint.trim().is_empty() {
            return Err(
                "proxy.agent_endpoint is required (set [proxy].agent_endpoint or \
                 AGATE__PROXY__AGENT_ENDPOINT)"
                    .into(),
            );
        }
        // Zero is a footgun, not a sensible "disable": a 0-byte body limit
        // rejects every request, and a 0s timeout fails the connection at once.
        if self.max_body_bytes == 0 {
            return Err("proxy.max_body_bytes must be greater than 0".into());
        }
        if self.connect_timeout_secs == 0 || self.read_timeout_secs == 0 {
            return Err(
                "proxy.connect_timeout_secs and proxy.read_timeout_secs must be greater than 0"
                    .into(),
            );
        }
        if self.max_concurrent_requests == 0 {
            return Err("proxy.max_concurrent_requests must be greater than 0".into());
        }
        // `0` here is not "unlimited" but the original unbounded-buffer bug: a
        // never-terminated frame would accumulate without a ceiling.
        if self.max_frame_bytes == 0 {
            return Err("proxy.max_frame_bytes must be greater than 0".into());
        }
        // A burst without a rate silently disables the limit (the middleware
        // short-circuits on a zero rate), which would leave DoS protection off
        // by surprise — fail fast instead.
        if self.rate_limit_per_second == 0 && self.rate_limit_burst != 0 {
            return Err("proxy.rate_limit_burst requires proxy.rate_limit_per_second > 0".into());
        }
        Ok(())
    }
}

impl Default for ProxySection {
    fn default() -> Self {
        Self {
            agent_endpoint: String::new(),
            bind: "0.0.0.0:8080".into(),
            connect_timeout_secs: 5,
            read_timeout_secs: 60,
            max_body_bytes: 1 << 20,
            api_key: None,
            api_keys: Vec::new(),
            max_concurrent_requests: 256,
            // Generous defaults that catch a runaway stream without tripping a
            // legitimate long run; `0` disables a limit.
            max_response_events: 100_000,
            max_response_bytes: 64 << 20,
            max_frame_bytes: 1 << 20,
            // Disabled by default: the peer IP is only meaningful when Agate
            // sees the real client (not behind an unconfigured load balancer),
            // so opt in once the deployment's ingress is understood.
            rate_limit_per_second: 0,
            rate_limit_burst: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ProxySection;

    fn valid() -> ProxySection {
        ProxySection {
            agent_endpoint: "http://agent/run".to_owned(),
            ..ProxySection::default()
        }
    }

    #[test]
    fn a_burst_without_a_rate_is_rejected() {
        let section = ProxySection {
            rate_limit_per_second: 0,
            rate_limit_burst: 20,
            ..valid()
        };
        let error = section
            .validate()
            .expect_err("a burst with no rate is invalid");
        assert!(error.contains("rate_limit_burst"), "{error}");
    }

    #[test]
    fn a_rate_with_or_without_a_burst_is_accepted() {
        let with_burst = ProxySection {
            rate_limit_per_second: 10,
            rate_limit_burst: 20,
            ..valid()
        };
        assert!(with_burst.validate().is_ok());

        let rate_only = ProxySection {
            rate_limit_per_second: 10,
            rate_limit_burst: 0,
            ..valid()
        };
        assert!(rate_only.validate().is_ok());
    }

    #[test]
    fn both_zero_disables_the_limit_and_is_valid() {
        assert!(valid().validate().is_ok());
    }
}
