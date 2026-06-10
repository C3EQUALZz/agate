//! DI handles for the proxy ports.
//!
//! `froodi` resolves *concrete* types, so it cannot hand back an `Arc<dyn Port>`
//! directly. The container builds the concrete adapter, wraps it in the
//! matching handle below, and registers the handle; the HTTP handler then
//! `Inject`s the handle (adapter-agnostic) and reads `.0`. The net effect:
//! presentation never names a concrete adapter, so swapping the upstream
//! client or the metrics recorder touches only the container.

use std::sync::Arc;

use crate::application::common::ports::{ProxyMetrics, UpstreamAgentClient};

/// The upstream agent client for the configured provider.
pub struct UpstreamAgentClientHandle(pub Arc<dyn UpstreamAgentClient>);

/// The data-plane metrics recorder.
pub struct ProxyMetricsHandle(pub Arc<dyn ProxyMetrics>);
