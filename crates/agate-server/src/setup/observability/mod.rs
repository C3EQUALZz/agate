//! Process-level observability, set up by the composition root from config.
//!
//! Logging is configurable today (console/JSON, level, on/off); metrics and
//! tracing connectors plug in here next, selected by `[observability]` config.

pub mod logging;
pub mod metrics;

pub use logging::init_logging;
pub use metrics::init_metrics;
