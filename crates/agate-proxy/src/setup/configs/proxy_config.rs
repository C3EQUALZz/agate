use std::time::Duration;

use crate::application::inspection::{InspectionSettings, MalformedEventMode, ResponseBudget};
use crate::domain::inspection::Budgets;

/// Which store backs the cross-run session-replay ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionMemoryBackend {
    /// A process-local ledger (single instance). State is lost on restart and
    /// not shared across replicas.
    InMemory,
    /// A shared Redis ledger (multiple proxy instances see the same session),
    /// at the given connection URL. State survives a restart and spans replicas.
    Redis(String),
}

/// Cross-run replay-memory configuration. Present on [`ProxyConfig`] means
/// enabled; absent means the policy is judged afresh every run.
#[derive(Clone, Debug)]
pub struct SessionMemoryConfig {
    /// Where the ledger lives.
    pub backend: SessionMemoryBackend,
    /// How long a session's quarantine survives without activity.
    pub ttl: Duration,
}

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
    /// Accepted API keys: a request is authenticated if its `X-API-Key` header
    /// matches **any** of these. Empty disables authentication (the proxy is
    /// open) — only sensible behind another guard. Holding more than one key at
    /// once is how rotation works: add the new key, migrate clients, drop the old.
    pub api_keys: Vec<String>,
    /// Maximum concurrently in-flight proxied runs; excess is shed with `503`.
    pub max_concurrent_requests: usize,
    /// What to do with a recognized-but-malformed response event (defaults to
    /// the secure [`Terminate`](MalformedEventMode::Terminate)).
    pub malformed_event_mode: MalformedEventMode,
    /// Per-run ceiling on the response stream (events / bytes); defaults to
    /// unlimited, the composition root applies the configured limits.
    pub response_budget: ResponseBudget,
    /// Sustained per-client-IP request rate (requests per second); `0` disables
    /// rate limiting.
    pub rate_limit_per_second: u32,
    /// Burst depth for the per-IP rate limit (largest instantaneous burst); `0`
    /// falls back to [`rate_limit_per_second`](Self::rate_limit_per_second).
    pub rate_limit_burst: u32,
    /// Cross-run replay memory: `Some` quarantines a tool denied in one run for
    /// the rest of the session (in-memory or Redis); `None` disables it (the
    /// policy is judged afresh every run).
    pub session_memory: Option<SessionMemoryConfig>,
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
            api_keys: Vec::new(),
            max_concurrent_requests: DEFAULT_MAX_CONCURRENT_REQUESTS,
            malformed_event_mode: MalformedEventMode::default(),
            response_budget: ResponseBudget::default(),
            rate_limit_per_second: 0,
            rate_limit_burst: 0,
            session_memory: None,
        }
    }

    /// The grouped stream-guard settings handed to the inspection pipeline:
    /// the default structural budgets plus this config's malformed-event mode
    /// and response budget.
    #[must_use]
    pub fn inspection_settings(&self) -> InspectionSettings {
        InspectionSettings {
            budgets: Budgets::default(),
            malformed_mode: self.malformed_event_mode,
            response_budget: self.response_budget,
        }
    }

    /// Override the per-run response budget (events / bytes).
    #[must_use]
    pub fn with_response_budget(mut self, response_budget: ResponseBudget) -> Self {
        self.response_budget = response_budget;
        self
    }

    /// Override the handling of recognized-but-malformed response events.
    #[must_use]
    pub fn with_malformed_event_mode(mut self, mode: MalformedEventMode) -> Self {
        self.malformed_event_mode = mode;
        self
    }

    /// Override the maximum number of concurrently in-flight proxied runs.
    #[must_use]
    pub fn with_concurrency_limit(mut self, max_concurrent_requests: usize) -> Self {
        self.max_concurrent_requests = max_concurrent_requests;
        self
    }

    /// Override the per-client-IP request-rate limit (`per_second` = 0 disables
    /// it; `burst` = 0 falls back to `per_second`).
    #[must_use]
    pub fn with_rate_limit(mut self, per_second: u32, burst: u32) -> Self {
        self.rate_limit_per_second = per_second;
        self.rate_limit_burst = burst;
        self
    }

    /// Override the cross-run session-replay memory (`Some` enables it with the
    /// given backend + TTL; `None` disables it).
    #[must_use]
    pub fn with_session_memory(mut self, session_memory: Option<SessionMemoryConfig>) -> Self {
        self.session_memory = session_memory;
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
        api_keys: Vec<String>,
    ) -> Self {
        self.connect_timeout = connect_timeout;
        self.read_timeout = read_timeout;
        self.max_body_bytes = max_body_bytes;
        self.api_keys = api_keys;
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
