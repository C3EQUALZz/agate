use std::sync::Arc;

use froodi::async_impl::Container;
use tokio::sync::mpsc::Receiver;
use tracing::{debug, error};

use agate_audit::application::common::messaging::{Dispatcher, Registry};
use agate_audit::application::common::ports::AuditMetrics;
use agate_audit::application::usecases::append_record::AppendRecord;
use agate_audit::domain::merkle::LogId;

/// Drains the audit channel, appending each record to one transparency log.
///
/// A single outbox is the log's sole writer, so the channel's FIFO order becomes
/// the log's append order. Each append runs in its own audit request scope (one
/// transaction, the audit context's commit boundary). A failed append is logged
/// and skipped: the audit trail must never stall the queue or take down the
/// proxy.
pub struct AuditOutbox {
    container: Container,
    registry: Arc<Registry<Container>>,
    log: LogId,
    metrics: Arc<dyn AuditMetrics>,
}

impl AuditOutbox {
    #[must_use]
    pub fn new(
        container: Container,
        registry: Arc<Registry<Container>>,
        log: LogId,
        metrics: Arc<dyn AuditMetrics>,
    ) -> Self {
        Self {
            container,
            registry,
            log,
            metrics,
        }
    }

    /// Run until the channel closes (every [`AuditLogSink`](super::AuditLogSink)
    /// has been dropped), appending each queued record in turn.
    pub async fn run(self, mut records: Receiver<Vec<u8>>) {
        debug!(log = %self.log.0, "audit outbox started");
        while let Some(record) = records.recv().await {
            self.append(record).await;
        }
        debug!(log = %self.log.0, "audit outbox channel closed; stopping");
    }

    async fn append(&self, record: Vec<u8>) {
        let scope = match self.container.clone().enter_build() {
            Ok(scope) => Arc::new(scope),
            Err(error) => {
                // The append never reaches the dispatcher, so the MetricsBehavior
                // cannot count it — record the drop here instead.
                error!(?error, "audit outbox: cannot open request scope");
                self.metrics.record_dropped();
                return;
            }
        };
        let dispatcher = Dispatcher::new(scope.clone(), self.registry.clone());
        // appended / dropped counters are emitted by the audit MetricsBehavior in
        // the dispatch pipeline; here we only log the per-record outcome.
        match dispatcher
            .send(AppendRecord {
                log: self.log,
                record,
            })
            .await
        {
            Ok(index) => debug!(log = %self.log.0, index = index.0, "appended audit record"),
            Err(error) => error!(?error, "audit outbox: append failed"),
        }
        scope.close().await;
    }
}
