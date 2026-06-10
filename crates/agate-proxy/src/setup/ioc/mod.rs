//! IoC composition: the `froodi` container of the proxy's adapters and the
//! inspector.

pub mod container;
/// Type-erased DI handles so the presentation layer injects ports, not the
/// concrete adapters behind them.
pub mod handles;

pub use container::{build_container, build_container_with};
pub use handles::{ProxyMetricsHandle, UpstreamAgentClientHandle};
