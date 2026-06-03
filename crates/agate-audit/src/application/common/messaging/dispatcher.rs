use std::sync::Arc;

use super::mediator::Mediator;
use super::registry::Registry;
use super::request::Request;

/// Sends a request through its pipeline by looking the handler and behaviors up
/// in the [`Registry`] and resolving them from the container (the bazario
/// `Dispatcher` role). One per request scope: it holds that scope's container.
///
/// The chain itself is run by [`Mediator`], reused per send.
pub struct Dispatcher<C> {
    container: Arc<C>,
    registry: Arc<Registry<C>>,
}

impl<C: Send + Sync + 'static> Dispatcher<C> {
    pub fn new(container: Arc<C>, registry: Arc<Registry<C>>) -> Self {
        Self {
            container,
            registry,
        }
    }

    pub async fn send<R: Request>(&self, request: R) -> R::Response {
        let handler = self
            .registry
            .resolve_handler::<R>(self.container.as_ref())
            .await
            .expect("no handler registered for this request type");
        let behaviors = self
            .registry
            .resolve_behaviors::<R>(self.container.as_ref())
            .await;
        Mediator::new(handler, behaviors).send(request).await
    }
}
