use std::sync::Arc;

use axum::routing::{get, post};
use axum::{Extension, Router};
use froodi::axum::setup_async_default;
use sqlx::PgPool;

use super::handlers;
use crate::infrastructure::di::{build_container, build_registry};

/// Build the HTTP application: the routes, the shared routing table (as an
/// extension), and the `froodi` per-request scope layer that gives each request
/// its own container — hence its own transaction.
pub fn build_app(pool: PgPool) -> Router {
    let container = build_container(pool);
    let registry = Arc::new(build_registry());

    let routes = Router::new()
        .route("/logs", post(handlers::create_log))
        .route("/logs/{id}/records", post(handlers::append_record))
        .route(
            "/logs/{id}/inclusion/{index}",
            get(handlers::inclusion_proof),
        )
        .route(
            "/logs/{id}/consistency/{first}",
            get(handlers::consistency_proof),
        )
        .layer(Extension(registry));

    setup_async_default(routes, container)
}
