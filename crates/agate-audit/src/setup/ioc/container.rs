use froodi::{DefaultScope::App, async_impl::Container, async_registry};

use super::providers::{
    agnostic_providers, dispatch_providers, handler_providers, postgres_providers,
};
use crate::setup::storage::Storage;

/// Build the IoC container for the connected `storage`, started at the App
/// scope. The backend's adapters are selected here (one `match` arm per
/// backend); everything else — the agnostic singletons, handlers, dispatch — is
/// backend-independent. Open a Request scope per request with
/// `container.clone().enter_build()` (the `froodi` axum layer does this per
/// HTTP request).
#[must_use]
pub fn build_container(storage: &Storage) -> Container {
    let backend = match storage {
        Storage::Postgres(pool) => postgres_providers(pool.clone()),
    };
    let ioc = async_registry! {
        extend(
            backend,
            agnostic_providers(),
            handler_providers(),
            dispatch_providers(),
        )
    };
    Container::new_with_start_scope(ioc, App)
}
