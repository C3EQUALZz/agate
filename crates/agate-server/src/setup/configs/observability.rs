use serde::{Deserialize, Serialize};

/// `[observability]` — logging now; metrics/tracing connectors land here next.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservabilityConfig {
    pub logging: LoggingConfig,
}

/// `[observability.logging]` — the console/structured log output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Install a subscriber at all. `false` silences logs entirely.
    pub enabled: bool,
    /// Console (`pretty`) or structured (`json`) output.
    pub format: LogFormat,
    /// Filter directive (e.g. `info`, `agate_proxy=debug,info`). `RUST_LOG`
    /// overrides it when set.
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            format: LogFormat::Pretty,
            level: "info".into(),
        }
    }
}

/// The log output format — one of the selectable logging "connectors".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogFormat {
    /// Human-readable, for a console/terminal.
    #[default]
    Pretty,
    /// One JSON object per line, for log shippers.
    Json,
}
