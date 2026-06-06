use std::sync::Arc;

use async_trait::async_trait;

use crate::application::common::messaging::{Behavior, Next, Request};
use crate::application::common::ports::AuditMetrics;
use crate::application::usecases::append_record::AppendRecord;

/// Pipeline behavior that records the outcome of an [`AppendRecord`]: one
/// `appended` on success, one `dropped` on failure.
///
/// Metric emission is application logic here, kept behind the [`AuditMetrics`]
/// port. Registered outermost on `AppendRecord` at the composition root, so it
/// observes the final result *after* the transaction behavior has committed or
/// rolled back — a commit failure is counted as a drop, not an append.
pub struct MetricsBehavior {
    metrics: Arc<dyn AuditMetrics>,
}

impl MetricsBehavior {
    pub fn new(metrics: Arc<dyn AuditMetrics>) -> Self {
        Self { metrics }
    }
}

#[async_trait]
impl Behavior<AppendRecord> for MetricsBehavior {
    async fn handle(
        &self,
        request: AppendRecord,
        next: Next<AppendRecord>,
    ) -> <AppendRecord as Request>::Response {
        let result = next.call(request).await;
        match &result {
            Ok(_) => self.metrics.record_appended(),
            Err(_) => self.metrics.record_dropped(),
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use uuid::Uuid;

    use super::*;
    use crate::application::common::messaging::{Mediator, RequestHandler};
    use crate::application::errors::AuditError;
    use crate::domain::merkle::{LeafIndex, LogId};

    #[derive(Default)]
    struct CountingMetrics {
        appended: AtomicUsize,
        dropped: AtomicUsize,
    }

    impl AuditMetrics for CountingMetrics {
        fn record_appended(&self) {
            self.appended.fetch_add(1, Ordering::SeqCst);
        }
        fn record_dropped(&self) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct OkHandler;
    #[async_trait]
    impl RequestHandler<AppendRecord> for OkHandler {
        async fn handle(&self, _request: AppendRecord) -> Result<LeafIndex, AuditError> {
            Ok(LeafIndex(0))
        }
    }

    struct ErrHandler;
    #[async_trait]
    impl RequestHandler<AppendRecord> for ErrHandler {
        async fn handle(&self, _request: AppendRecord) -> Result<LeafIndex, AuditError> {
            Err(AuditError::Storage("boom".to_string()))
        }
    }

    fn command() -> AppendRecord {
        AppendRecord {
            log: LogId(Uuid::nil()),
            record: Vec::new(),
        }
    }

    #[tokio::test]
    async fn counts_an_appended_record_on_success() {
        let metrics = Arc::new(CountingMetrics::default());
        let behaviors: Vec<Arc<dyn Behavior<AppendRecord>>> =
            vec![Arc::new(MetricsBehavior::new(metrics.clone()))];
        let mediator = Mediator::new(Arc::new(OkHandler), behaviors);

        assert!(mediator.send(command()).await.is_ok());
        assert_eq!(metrics.appended.load(Ordering::SeqCst), 1);
        assert_eq!(metrics.dropped.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn counts_a_dropped_record_on_error() {
        let metrics = Arc::new(CountingMetrics::default());
        let behaviors: Vec<Arc<dyn Behavior<AppendRecord>>> =
            vec![Arc::new(MetricsBehavior::new(metrics.clone()))];
        let mediator = Mediator::new(Arc::new(ErrHandler), behaviors);

        assert!(mediator.send(command()).await.is_err());
        assert_eq!(metrics.appended.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.dropped.load(Ordering::SeqCst), 1);
    }
}
