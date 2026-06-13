use std::sync::Arc;

use froodi::async_impl::Container;

use agate_audit::application::common::messaging::{Dispatcher, Registry, Request};
use agate_audit::application::errors::AuditError;

use crate::infrastructure::audit::ScopeError;

/// Run one audit command in its own request scope (one transaction): open a
/// scope, dispatch through the pipeline, then close it. The scope/container
/// lifecycle lives here so the outbox and checkpoint scheduler don't carry it —
/// both their adapters ([`ScopedAppender`](super::ScopedAppender),
/// [`ScopedIssuer`](super::ScopedIssuer)) are one call to this.
pub(crate) async fn dispatch_in_scope<R, T>(
    container: &Container,
    registry: &Arc<Registry<Container>>,
    request: R,
) -> Result<T, ScopeError>
where
    R: Request<Response = Result<T, AuditError>>,
{
    let scope = container
        .clone()
        .enter_build()
        .map_err(|error| ScopeError::Unavailable(format!("{error:?}")))?;
    let scope = Arc::new(scope);
    let dispatcher = Dispatcher::new(scope.clone(), registry.clone());
    let result = dispatcher.send(request).await;
    scope.close().await;
    result.map_err(ScopeError::Pipeline)
}
