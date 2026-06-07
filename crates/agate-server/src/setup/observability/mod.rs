//! Process-level observability, set up by the composition root from config:
//! logging (console/JSON, level, on/off), metrics (Prometheus), and OTLP
//! tracing — all selected by `[observability]` config.

pub mod logging;
pub mod metrics;
pub mod tracing;

pub use logging::init_logging;
pub use metrics::init_metrics;
