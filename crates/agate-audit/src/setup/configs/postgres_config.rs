/// PostgreSQL connection settings, loaded from the environment.
#[derive(Clone, Debug)]
pub struct PostgresConfig {
    url: String,
}

impl PostgresConfig {
    /// Build from an explicit connection URL (e.g. assembled by a composition
    /// root from a config file).
    #[must_use]
    pub fn new(url: String) -> Self {
        Self { url }
    }

    /// Reads `DATABASE_URL`. Panics if it is unset — a startup misconfiguration.
    #[must_use]
    pub fn from_env() -> Self {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        Self { url }
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}
