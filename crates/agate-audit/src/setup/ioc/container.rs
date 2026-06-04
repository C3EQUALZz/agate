use froodi::{DefaultScope::App, async_impl::Container, async_registry};
use sqlx::PgPool;

use super::providers::{handler_providers, infrastructure_providers};

/// Build the IoC container, started at the App scope. Open a Request scope per
/// request with `container.clone().enter_build()`.
#[must_use]
pub fn build_container(pool: PgPool) -> Container {
    let ioc = async_registry! {
        extend(infrastructure_providers(pool), handler_providers())
    };
    Container::new_with_start_scope(ioc, App)
}
