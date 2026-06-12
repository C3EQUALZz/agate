//! Typed configuration: a layered TOML + environment load.
//!
//! [`load`] reads built-in defaults, then `agate.toml`, then `AGATE__*`
//! environment overrides into an [`AppConfig`]; the composition root maps each
//! section onto the bounded contexts' own config types.

pub mod app_config;
pub mod audit_section;
pub mod loader;
pub mod observability;
pub mod policy_section;
pub mod proxy_section;
pub mod tls;

pub use app_config::AppConfig;
pub use loader::load;
pub use observability::{
    LogFormat, LoggingConfig, MetricsConfig, MetricsExporter, ObservabilityConfig, TracingConfig,
};
pub use policy_section::ToolMode;
pub use tls::TlsConfig;
