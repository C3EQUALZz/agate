//! IoC composition: the `froodi` container and the messaging routing table.
//!
//! Handlers and behaviors are registered as plain providers by their concrete
//! type (see [`providers`]); the [`Registry`](crate::application::common::messaging::Registry)
//! maps request types to them and the
//! [`Dispatcher`](crate::application::common::messaging::Dispatcher) resolves
//! them through the [`Resolve`] bridge implemented here for the container.

pub mod container;
pub mod handles;
pub mod providers;
pub mod registry;

pub use container::build_container;
pub use registry::build_registry;

use std::sync::Arc;

use async_trait::async_trait;
use froodi::async_impl::Container;

use crate::application::common::messaging::{Resolve, ResolveError};

/// Bridges the container-agnostic messaging layer to `froodi`: `Resolve<T>`
/// delegates to `Container::get::<T>()`.
#[async_trait]
impl<T: Send + Sync + 'static> Resolve<T> for Container {
    async fn resolve(&self) -> Result<Arc<T>, ResolveError> {
        self.get::<T>()
            .await
            .map_err(|error| ResolveError(error.to_string()))
    }
}
