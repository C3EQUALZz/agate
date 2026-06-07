use crate::infrastructure::persistence::postgres::PoolConfig;

/// PostgreSQL connection settings: the URL plus pool sizing and the startup
/// connect-retry policy ([`PoolConfig`]).
#[derive(Clone, Debug)]
pub struct PostgresConfig {
    url: String,
    pool: PoolConfig,
}

impl PostgresConfig {
    /// Build from an explicit connection URL (e.g. assembled by a composition
    /// root from a config file), with default pool/retry settings.
    #[must_use]
    pub fn new(url: String) -> Self {
        Self {
            url,
            pool: PoolConfig::default(),
        }
    }

    /// Reads `DATABASE_URL`. Panics if it is unset — a startup misconfiguration.
    #[must_use]
    pub fn from_env() -> Self {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        Self::new(url)
    }

    /// Override the pool sizing and connect-retry policy.
    #[must_use]
    pub fn with_pool(mut self, pool: PoolConfig) -> Self {
        self.pool = pool;
        self
    }

    /// The connection URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The pool sizing and connect-retry policy.
    pub fn pool(&self) -> &PoolConfig {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::PostgresConfig;

    #[test]
    fn new_uses_default_pool_settings() {
        let config = PostgresConfig::new("postgres://localhost/agate".to_owned());
        assert_eq!(config.url(), "postgres://localhost/agate");
        assert_eq!(config.pool().max_connections, 10);
        assert_eq!(config.pool().connect_max_retries, 10);
    }
}
