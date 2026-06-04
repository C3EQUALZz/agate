use super::http_config::HttpConfig;
use super::postgres_config::PostgresConfig;

/// Aggregates the service configuration, loaded from the environment.
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub postgres: PostgresConfig,
    pub http: HttpConfig,
}

impl AppConfig {
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            postgres: PostgresConfig::from_env(),
            http: HttpConfig::from_env(),
        }
    }
}
