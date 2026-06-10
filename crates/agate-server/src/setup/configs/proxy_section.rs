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
    /// unlimited). A runaway agent over this is cut off with a `RUN_ERROR`.
    pub max_response_events: usize,
    /// Per-run ceiling on response bytes streamed to the client (`0` =
    /// unlimited).
    pub max_response_bytes: usize,
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
        }
    }
}
