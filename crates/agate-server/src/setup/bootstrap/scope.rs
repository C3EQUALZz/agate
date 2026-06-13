use std::sync::Arc;

use froodi::async_impl::Container;

use agate_audit::application::common::messaging::{Dispatcher, Registry, Request};
use agate_audit::application::errors::AuditError;

use crate::infrastructure::audit::ScopeError;

/// Runs audit commands each in their own request scope (one transaction): open a
/// scope, dispatch through the pipeline, close it. The scope/container lifecycle
/// lives here once, so the outbox and checkpoint scheduler stay container-
/// agnostic — their adapters ([`ScopedAppender`](super::ScopedAppender),
/// [`ScopedIssuer`](super::ScopedIssuer)) just hold one of these and forward a
/// typed command.
pub(crate) struct ScopedDispatcher {
    container: Container,
    registry: Arc<Registry<Container>>,
}

impl ScopedDispatcher {
    pub(crate) fn new(container: Container, registry: Arc<Registry<Container>>) -> Self {
        Self {
            container,
            registry,
        }
    }

    /// Dispatch one command in a fresh scope, mapping a scope-open failure and a
    /// pipeline failure onto [`ScopeError`].
    pub(crate) async fn dispatch<R, T>(&self, request: R) -> Result<T, ScopeError>
    where
        R: Request<Response = Result<T, AuditError>>,
    {
        let scope = self
            .container
            .clone()
            .enter_build()
            .map_err(|error| ScopeError::Unavailable(format!("{error:?}")))?;
        let scope = Arc::new(scope);
        let dispatcher = Dispatcher::new(scope.clone(), self.registry.clone());
        let result = dispatcher.send(request).await;
        scope.close().await;
        result.map_err(ScopeError::Pipeline)
    }
}
