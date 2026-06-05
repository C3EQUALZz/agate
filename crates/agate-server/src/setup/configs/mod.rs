//! Typed configuration: a layered TOML + environment load.
//!
//! [`load`] reads built-in defaults, then `agate.toml`, then `AGATE__*`
//! environment overrides into an [`AppConfig`]; the composition root maps each
//! section onto the bounded contexts' own config types.

pub mod app_config;
pub mod loader;
pub mod observability;

pub use app_config::{AppConfig, ToolMode};
pub use loader::load;
pub use observability::{LogFormat, LoggingConfig, ObservabilityConfig};
