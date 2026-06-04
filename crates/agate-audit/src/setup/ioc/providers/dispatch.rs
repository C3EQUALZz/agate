//! The messaging dispatch providers: the routing table (App singleton) and the
//! per-request [`Dispatcher`].
//!
//! The dispatcher holds the request-scope container, which `froodi` does not
//! otherwise expose to a provider — so [`RequestContainer`] is a small custom
//! [`DependencyResolver`] that hands the resolving container back. Handlers then
//! just take `Inject<Dispatcher<Container>>`.

use std::sync::Arc;

use froodi::{
    Container as SyncContainer,
    DefaultScope::{App, Request},
    DependencyResolver, Inject, InstantiatorResult, ResolveErrorKind,
    async_impl::{Container, RegistryWithSync},
    async_registry, registry,
};

use crate::application::common::messaging::{Dispatcher, Registry};
use crate::setup::ioc::build_registry;

/// Resolves to the request-scope container itself (a clone), so a provider can
/// depend on it. Only the async container is used (`setup_async_default`), so
/// the sync path is unreachable.
pub struct RequestContainer(pub Container);

impl DependencyResolver for RequestContainer {
    type Error = ResolveErrorKind;

    fn resolve(_container: &SyncContainer) -> Result<Self, Self::Error> {
        unreachable!("the dispatcher is resolved only from the async container")
    }

    async fn resolve_async(container: &Container) -> Result<Self, Self::Error> {
        Ok(Self(container.clone()))
    }
}

/// The routing table (App) and the dispatcher bound to each request scope.
pub(crate) fn dispatch_providers() -> RegistryWithSync {
    async_registry! {
        scope(Request) [
            provide(provide_dispatcher),
        ],
        extend(registry! {
            scope(App) [
                provide(|| Ok(build_registry())),
            ]
        }),
    }
}

async fn provide_dispatcher(
    RequestContainer(container): RequestContainer,
    Inject(registry): Inject<Registry<Container>>,
) -> InstantiatorResult<Dispatcher<Container>> {
    Ok(Dispatcher::new(Arc::new(container), registry))
}
