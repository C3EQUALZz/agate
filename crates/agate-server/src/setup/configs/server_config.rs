use agate_audit::setup::configs::PostgresConfig;
use agate_proxy::setup::configs::ProxyConfig;

/// Configuration for the whole server: the proxy data plane plus the Postgres
/// store backing the audit transparency log. Each part reuses the bounded
/// context's own config type, so this only composes them.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub proxy: ProxyConfig,
    pub postgres: PostgresConfig,
}

impl ServerConfig {
    /// Reads the proxy's `AGENT_ENDPOINT`/`BIND_ADDR` and the audit's
    /// `DATABASE_URL`.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            proxy: ProxyConfig::from_env(),
            postgres: PostgresConfig::from_env(),
        }
    }
}
