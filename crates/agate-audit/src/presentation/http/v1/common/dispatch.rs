use std::sync::Arc;

use froodi::async_impl::Container;

use crate::application::common::messaging::{Dispatcher, Registry};

/// The routing table, shared as an axum extension across requests.
pub type SharedRegistry = Arc<Registry<Container>>;

/// Build a dispatcher over this request's `froodi` scope (its own transaction)
/// — the request-scope container is supplied by the `froodi` axum layer.
#[must_use]
pub fn dispatcher(container: Container, registry: SharedRegistry) -> Dispatcher<Container> {
    Dispatcher::new(Arc::new(container), registry)
}
