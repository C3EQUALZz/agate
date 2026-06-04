/// Proxy configuration, loaded from the environment.
#[derive(Clone, Debug)]
pub struct ProxyConfig {
    /// Upstream agent run endpoint the proxy forwards to.
    pub agent_endpoint: String,
    /// Address the proxy listens on.
    pub bind_addr: String,
}

impl ProxyConfig {
    #[must_use]
    pub fn new(agent_endpoint: String, bind_addr: String) -> Self {
        Self {
            agent_endpoint,
            bind_addr,
        }
    }

    /// Reads `AGENT_ENDPOINT` (required) and `BIND_ADDR` (default `0.0.0.0:8080`).
    #[must_use]
    pub fn from_env() -> Self {
        let agent_endpoint = std::env::var("AGENT_ENDPOINT").expect("AGENT_ENDPOINT must be set");
        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        Self {
            agent_endpoint,
            bind_addr,
        }
    }
}
