use axum::Router;
use froodi::axum::setup_async_default;

use crate::presentation::http::health;
use crate::presentation::http::v1::routes::log;
use crate::setup::ioc::build_container;
use crate::setup::storage::Storage;

/// Assemble the HTTP application: the routes and the `froodi` per-request scope
/// layer that gives each request its own container — hence its own transaction.
/// The dispatcher (and routing table) are resolved from that container, so
/// handlers just take `Inject<Dispatcher>`.
pub fn build_app(storage: &Storage) -> Router {
    let container = build_container(storage);

    let routes = Router::new().merge(health::router()).merge(log::router());

    setup_async_default(routes, container)
}
