use std::sync::Arc;

use axum::Router;
use froodi::axum::setup_async_default;

use crate::application::common::ports::{AuditSink, PolicyPort};
use crate::presentation::http;
use crate::setup::configs::ProxyConfig;
use crate::setup::ioc::{build_container, build_container_with};

/// Assemble the HTTP application with the default adapters (allow-all policy,
/// no-op audit): the reverse-proxy routes and the `froodi` per-request scope
/// layer that resolves the inspector and agent client.
pub fn build_app(config: ProxyConfig) -> Router {
    let container = build_container(config);
    setup_async_default(http::router(), container)
}

/// Assemble the HTTP application with an explicit policy and audit sink — used
/// by the top-level server to wire real adapters in place of the defaults.
pub fn build_app_with(
    config: ProxyConfig,
    policy: Arc<dyn PolicyPort>,
    audit: Arc<dyn AuditSink>,
) -> Router {
    let container = build_container_with(config, policy, audit);
    setup_async_default(http::router(), container)
}
