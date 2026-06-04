use axum::Router;
use froodi::axum::setup_async_default;

use crate::presentation::http;
use crate::setup::configs::ProxyConfig;
use crate::setup::ioc::build_container;

/// Assemble the HTTP application: the reverse-proxy routes and the `froodi`
/// per-request scope layer that resolves the inspector and agent client.
pub fn build_app(config: ProxyConfig) -> Router {
    let container = build_container(config);
    setup_async_default(http::router(), container)
}
