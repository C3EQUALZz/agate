use serde::{Deserialize, Serialize};

/// `[observability]` — logging, metrics, and tracing connectors.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservabilityConfig {
    pub logging: LoggingConfig,
    pub metrics: MetricsConfig,
    pub tracing: TracingConfig,
}

/// `[observability.tracing]` — OTLP trace export (the third observability pillar,
/// beside logs and metrics).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TracingConfig {
    /// Export spans via OTLP at all. `false` leaves tracing as log spans only
    /// (no exporter installed).
    pub enabled: bool,
    /// OTLP gRPC endpoint of the collector (e.g. an OpenTelemetry Collector).
    pub endpoint: String,
    /// `service.name` reported on exported spans.
    pub service_name: String,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: "http://localhost:4317".into(),
            service_name: "agate-server".into(),
        }
    }
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

/// `[observability.metrics]` — a Prometheus scrape endpoint, on its own port so
/// it stays off the public data-plane port.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MetricsConfig {
    /// Install a metrics recorder + exporter at all. `false` makes every metric
    /// a no-op (the `metrics` facade does nothing without a recorder).
    pub enabled: bool,
    /// Which exporter to expose the metrics through.
    pub exporter: MetricsExporter,
    /// Address the Prometheus `/metrics` endpoint listens on (its own port,
    /// scraped from inside the network — not the client-facing proxy port).
    pub bind: String,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            exporter: MetricsExporter::Prometheus,
            bind: "0.0.0.0:9090".into(),
        }
    }
}

/// The metrics "connector" — selectable like the logging format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetricsExporter {
    /// A Prometheus text endpoint (`/metrics`) on its own port.
    #[default]
    Prometheus,
    /// No exporter (metrics disabled even if `enabled = true`).
    None,
}
