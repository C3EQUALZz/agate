use std::sync::Arc;

use async_trait::async_trait;
use froodi::async_impl::Container;

use agate_audit::application::common::messaging::Registry;
use agate_audit::application::usecases::append_record::AppendRecord;
use agate_audit::domain::merkle::{LeafIndex, LogId};

use super::scope::ScopedDispatcher;
use crate::infrastructure::audit::{RecordAppender, ScopeError};

/// The composition-root [`RecordAppender`]: appends each record in its own audit
/// request scope (one transaction). Knowing the container and the scope
/// lifecycle is exactly the knowledge kept out of the outbox.
pub struct ScopedAppender(ScopedDispatcher);

impl ScopedAppender {
    #[must_use]
    pub fn new(container: Container, registry: Arc<Registry<Container>>) -> Self {
        Self(ScopedDispatcher::new(container, registry))
    }
}

#[async_trait]
impl RecordAppender for ScopedAppender {
    async fn append(&self, log: LogId, record: Vec<u8>) -> Result<LeafIndex, ScopeError> {
        self.0
            .dispatch::<AppendRecord, LeafIndex>(AppendRecord { log, record })
            .await
    }
}
