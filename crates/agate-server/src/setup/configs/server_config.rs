use agate_audit::setup::configs::PostgresConfig;
use agate_proxy::setup::configs::ProxyConfig;

use super::policy_config::PolicyConfig;

/// Configuration for the whole server: the proxy data plane, the Postgres store
/// backing the audit transparency log, and the policy rules. Each part reuses
/// the bounded context's own config type, so this only composes them.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub proxy: ProxyConfig,
    pub postgres: PostgresConfig,
    pub policy: PolicyConfig,
}

impl ServerConfig {
    /// Reads the proxy's `AGENT_ENDPOINT`/`BIND_ADDR`, the audit's
    /// `DATABASE_URL`, and the `POLICY_*` rules.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            proxy: ProxyConfig::from_env(),
            postgres: PostgresConfig::from_env(),
            policy: PolicyConfig::from_env(),
        }
    }
}
