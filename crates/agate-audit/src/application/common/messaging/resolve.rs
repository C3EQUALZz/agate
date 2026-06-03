use std::sync::Arc;

use async_trait::async_trait;

/// Bridge from the messaging layer to a DI container: build one dependency by
/// its concrete type. Implemented for the IoC container at the composition
/// root, so [`Registry`](super::Registry) and [`Dispatcher`](super::Dispatcher)
/// stay container-agnostic (the bazario `Resolver` role).
#[async_trait]
pub trait Resolve<T: Send + Sync + 'static>: Send + Sync {
    async fn resolve(&self) -> Result<Arc<T>, ResolveError>;
}

/// A dependency could not be resolved — a composition-root wiring bug, not a
/// recoverable runtime condition.
#[derive(Debug)]
pub struct ResolveError(pub String);

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to resolve dependency: {}", self.0)
    }
}

impl std::error::Error for ResolveError {}
