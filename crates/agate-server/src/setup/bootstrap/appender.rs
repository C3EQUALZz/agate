use std::sync::Arc;

use async_trait::async_trait;
use froodi::async_impl::Container;

use agate_audit::application::common::messaging::{Dispatcher, Registry};
use agate_audit::application::usecases::append_record::AppendRecord;
use agate_audit::domain::merkle::{LeafIndex, LogId};

use crate::infrastructure::audit::{AppendError, RecordAppender};

/// The composition-root [`RecordAppender`]: opens one audit request scope per
/// append (one transaction) and dispatches [`AppendRecord`] through the
/// pipeline. Knowing the container and the scope lifecycle is exactly the
/// knowledge kept out of the outbox.
pub struct ScopedAppender {
    container: Container,
    registry: Arc<Registry<Container>>,
}

impl ScopedAppender {
    #[must_use]
    pub fn new(container: Container, registry: Arc<Registry<Container>>) -> Self {
        Self {
            container,
            registry,
        }
    }
}

#[async_trait]
impl RecordAppender for ScopedAppender {
    async fn append(&self, log: LogId, record: Vec<u8>) -> Result<LeafIndex, AppendError> {
        let scope = self
            .container
            .clone()
            .enter_build()
            .map_err(|error| AppendError::ScopeUnavailable(format!("{error:?}")))?;
        let scope = Arc::new(scope);
        let dispatcher = Dispatcher::new(scope.clone(), self.registry.clone());
        let result = dispatcher.send(AppendRecord { log, record }).await;
        scope.close().await;
        result.map_err(AppendError::Pipeline)
    }
}
