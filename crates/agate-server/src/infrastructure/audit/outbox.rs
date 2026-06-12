use std::sync::Arc;

use tokio::sync::mpsc::Receiver;
use tracing::{debug, error};

use agate_audit::application::common::ports::AuditMetrics;
use agate_audit::domain::merkle::LogId;

use super::appender::{AppendError, RecordAppender};

/// Drains the audit channel, appending each record to one transparency log.
///
/// A single outbox is the log's sole writer, so the channel's FIFO order becomes
/// the log's append order. Appends go through the [`RecordAppender`] port (one
/// request scope and transaction per record). A failed append is logged and
/// skipped: the audit trail must never stall the queue or take down the proxy.
pub struct AuditOutbox {
    appender: Arc<dyn RecordAppender>,
    log: LogId,
    metrics: Arc<dyn AuditMetrics>,
}

impl AuditOutbox {
    #[must_use]
    pub fn new(
        appender: Arc<dyn RecordAppender>,
        log: LogId,
        metrics: Arc<dyn AuditMetrics>,
    ) -> Self {
        Self {
            appender,
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
        // appended / dropped counters are emitted by the audit MetricsBehavior in
        // the dispatch pipeline; a record that never reached the pipeline is the
        // one drop only the outbox can count.
        match self.appender.append(self.log, record).await {
            Ok(index) => debug!(log = %self.log.0, index = index.0, "appended audit record"),
            Err(AppendError::ScopeUnavailable(error)) => {
                error!(%error, "audit outbox: cannot open request scope");
                self.metrics.record_dropped();
            }
            Err(AppendError::Pipeline(error)) => error!(?error, "audit outbox: append failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    use agate_audit::application::common::ports::AuditMetrics;
    use agate_audit::application::errors::AuditError;
    use agate_audit::domain::merkle::{LeafIndex, LogId};

    use super::{AppendError, AuditOutbox, RecordAppender};

    #[derive(Default)]
    struct CountingMetrics {
        dropped: AtomicUsize,
    }

    impl AuditMetrics for CountingMetrics {
        fn record_appended(&self) {}
        fn record_dropped(&self) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }

    enum Outcome {
        Appended,
        ScopeUnavailable,
        PipelineFailure,
    }

    struct FakeAppender {
        outcome: Outcome,
        appended: Mutex<Vec<Vec<u8>>>,
    }

    impl FakeAppender {
        fn new(outcome: Outcome) -> Arc<Self> {
            Arc::new(Self {
                outcome,
                appended: Mutex::new(Vec::new()),
            })
        }
    }

    #[async_trait]
    impl RecordAppender for FakeAppender {
        async fn append(&self, log: LogId, record: Vec<u8>) -> Result<LeafIndex, AppendError> {
            match self.outcome {
                Outcome::Appended => {
                    let mut appended = self.appended.lock().unwrap();
                    appended.push(record);
                    Ok(LeafIndex(appended.len() as u64 - 1))
                }
                Outcome::ScopeUnavailable => {
                    Err(AppendError::ScopeUnavailable("container closed".into()))
                }
                Outcome::PipelineFailure => {
                    Err(AppendError::Pipeline(AuditError::LogNotFound(log)))
                }
            }
        }
    }

    fn outbox(appender: Arc<FakeAppender>, metrics: Arc<CountingMetrics>) -> AuditOutbox {
        AuditOutbox::new(appender, LogId(Uuid::nil()), metrics)
    }

    #[tokio::test]
    async fn drains_the_channel_in_fifo_order() {
        let appender = FakeAppender::new(Outcome::Appended);
        let (tx, rx) = mpsc::channel::<Vec<u8>>(4);
        for record in [b"one".to_vec(), b"two".to_vec(), b"three".to_vec()] {
            tx.send(record).await.unwrap();
        }
        drop(tx);

        outbox(appender.clone(), Arc::default()).run(rx).await;

        let recorded = appender.appended.lock().unwrap();
        assert_eq!(
            *recorded,
            vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
        );
    }

    #[tokio::test]
    async fn unavailable_scope_counts_a_drop_and_keeps_draining() {
        let appender = FakeAppender::new(Outcome::ScopeUnavailable);
        let metrics = Arc::new(CountingMetrics::default());
        let (tx, rx) = mpsc::channel::<Vec<u8>>(4);
        tx.send(b"one".to_vec()).await.unwrap();
        tx.send(b"two".to_vec()).await.unwrap();
        drop(tx);

        outbox(appender, metrics.clone()).run(rx).await;

        assert_eq!(metrics.dropped.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn pipeline_failure_is_skipped_without_counting_a_drop() {
        let appender = FakeAppender::new(Outcome::PipelineFailure);
        let metrics = Arc::new(CountingMetrics::default());
        let (tx, rx) = mpsc::channel::<Vec<u8>>(4);
        tx.send(b"one".to_vec()).await.unwrap();
        drop(tx);

        outbox(appender, metrics.clone()).run(rx).await;

        // The MetricsBehavior inside the pipeline owns this counter.
        assert_eq!(metrics.dropped.load(Ordering::SeqCst), 0);
    }
}
