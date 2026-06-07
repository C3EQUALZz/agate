use std::time::Duration;

/// How long to wait to establish a connection to the upstream agent before
/// failing fast. Kept short so an unreachable agent surfaces quickly.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Idle timeout between chunks of the upstream SSE response. This is *not* an
/// overall request deadline — a healthy stream can run indefinitely — only a
/// guard against a stalled upstream.
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_mins(1);
/// Maximum accepted request body size (1 MiB) — a `RunAgentInput` is small.
pub const DEFAULT_MAX_BODY_BYTES: usize = 1 << 20;
/// Maximum concurrently in-flight proxied runs. Each holds an upstream
/// connection for its full stream, so this bounds memory/connection pressure;
/// requests over the cap are shed with `503` rather than queued unboundedly.
pub const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 256;

/// Proxy configuration.
#[derive(Clone, Debug)]
pub struct ProxyConfig {
    /// Upstream agent run endpoint the proxy forwards to.
    pub agent_endpoint: String,
    /// Address the proxy listens on.
    pub bind_addr: String,
    /// Connect timeout to the upstream agent (fail fast when unreachable).
    pub connect_timeout: Duration,
    /// Idle read timeout between upstream response chunks.
    pub read_timeout: Duration,
    /// Maximum accepted request body size, in bytes.
    pub max_body_bytes: usize,
    /// Optional API key required on the `X-API-Key` header. `None` disables
    /// authentication (the proxy is open) — only sensible behind another guard.
    pub api_key: Option<String>,
    /// Maximum concurrently in-flight proxied runs; excess is shed with `503`.
    pub max_concurrent_requests: usize,
}

impl ProxyConfig {
    /// A config with the ingress defaults (timeouts, body limit, no API key).
    #[must_use]
    pub fn new(agent_endpoint: String, bind_addr: String) -> Self {
        Self {
            agent_endpoint,
            bind_addr,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            read_timeout: DEFAULT_READ_TIMEOUT,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            api_key: None,
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
        }
    }

    /// Override the maximum number of concurrently in-flight proxied runs.
    #[must_use]
    pub fn with_concurrency_limit(mut self, max_concurrent_requests: usize) -> Self {
        self.max_concurrent_requests = max_concurrent_requests;
        self
    }

    /// Override the ingress-hardening knobs (timeouts, body limit, API key),
    /// keeping the endpoint and bind address. Used by the composition root to
    /// apply the mounted configuration.
    #[must_use]
    pub fn with_ingress(
        mut self,
        connect_timeout: Duration,
        read_timeout: Duration,
        max_body_bytes: usize,
        api_key: Option<String>,
    ) -> Self {
        self.connect_timeout = connect_timeout;
        self.read_timeout = read_timeout;
        self.max_body_bytes = max_body_bytes;
        self.api_key = api_key;
        self
    }

    /// Reads `AGENT_ENDPOINT` (required) and `BIND_ADDR` (default `0.0.0.0:8080`),
    /// with the ingress defaults.
    #[must_use]
    pub fn from_env() -> Self {
        let agent_endpoint = std::env::var("AGENT_ENDPOINT").expect("AGENT_ENDPOINT must be set");
        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        Self::new(agent_endpoint, bind_addr)
    }
}
