//! Typed configuration, loaded from the environment.

pub mod proxy_config;

pub use proxy_config::{ProxyConfig, SessionMemoryBackend, SessionMemoryConfig};
