/// HTTP server settings, loaded from the environment.
#[derive(Clone, Debug)]
pub struct HttpConfig {
    pub bind_addr: String,
}

impl HttpConfig {
    /// Reads `BIND_ADDR`, defaulting to `0.0.0.0:8080`.
    #[must_use]
    pub fn from_env() -> Self {
        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        Self { bind_addr }
    }
}
