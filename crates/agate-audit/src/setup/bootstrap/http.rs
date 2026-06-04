use std::sync::Arc;

use axum::{Extension, Router};
use froodi::axum::setup_async_default;
use sqlx::PgPool;

use crate::presentation::http::health;
use crate::presentation::http::v1::routes::log;
use crate::setup::ioc::{build_container, build_registry};

/// Assemble the HTTP application: the routes, the shared routing table (as an
/// extension), and the `froodi` per-request scope layer that gives each request
/// its own container — hence its own transaction.
pub fn build_app(pool: PgPool) -> Router {
    let container = build_container(pool);
    let registry = Arc::new(build_registry());

    let routes = Router::new()
        .merge(health::router())
        .merge(log::router())
        .layer(Extension(registry));

    setup_async_default(routes, container)
}
