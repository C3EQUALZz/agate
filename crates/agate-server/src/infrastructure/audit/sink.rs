use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::error::TrySendError;

use agate_audit::application::common::ports::AuditMetrics;
use agate_proxy::application::common::ports::AuditSink;
use agate_proxy::application::inspection::InspectionContext;
use agate_proxy::domain::inspection::{AgentEvent, Verdict};

use super::record::encode_record;

/// What the sink does when the bounded outbox is full — the operator's
/// completeness-vs-availability choice for the audit write path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FullPolicy {
    /// Apply backpressure: await a free slot, slowing the forwarding path so no
    /// record is lost. The default — a transparency log values completeness.
    #[default]
    Block,
    /// Shed: drop the record (loudly logged + counted) so the proxy keeps
    /// serving. Trades a tamper-evidence gap for availability.
    Shed,
}

/// The proxy-side [`AuditSink`]: encodes each inspected event and enqueues it on
/// the outbox channel. Each enqueue reports the outbox depth so operators can
/// see backpressure building; what happens when the channel is full is the
/// configured [`FullPolicy`].
///
/// A record is dropped on a closed channel (shutdown, after the outbox task
/// stops) and, under [`FullPolicy::Shed`], on a full channel. Every drop is
/// logged and counted — never silent — though the caller cannot observe it
/// (`record` returns `()`, the outbox contract).
pub struct AuditLogSink {
    tx: Sender<Vec<u8>>,
    metrics: Arc<dyn AuditMetrics>,
    on_full: FullPolicy,
}

impl AuditLogSink {
    #[must_use]
    pub fn new(tx: Sender<Vec<u8>>, metrics: Arc<dyn AuditMetrics>, on_full: FullPolicy) -> Self {
        Self {
            tx,
            metrics,
            on_full,
        }
    }
}

#[async_trait]
impl AuditSink for AuditLogSink {
    async fn record(
        &self,
        context: &InspectionContext,
        event: &AgentEvent,
        verdict: &Verdict<AgentEvent>,
    ) {
        let record = encode_record(context, event, verdict);
        // Surface saturation before enqueuing: used = capacity - free slots.
        let capacity = self.tx.max_capacity();
        self.metrics
            .observe_outbox_depth(capacity - self.tx.capacity(), capacity);

        // Enqueue per the full-queue policy, then handle the outcome through one
        // path so the drop legs cannot diverge in what they log and count.
        let outcome = match self.on_full {
            FullPolicy::Block => self.tx.send(record).await.map_err(|_| Dropped::Closed),
            FullPolicy::Shed => self.tx.try_send(record).map_err(|error| match error {
                TrySendError::Full(_) => Dropped::Shed,
                TrySendError::Closed(_) => Dropped::Closed,
            }),
        };
        match outcome {
            Ok(()) => {
                tracing::debug!(run = %context.run, "enqueued inspected event to the audit outbox");
            }
            Err(dropped) => {
                match dropped {
                    Dropped::Shed => tracing::error!(
                        run = %context.run,
                        "audit outbox full; SHEDDING an inspected record — transparency-log gap"
                    ),
                    Dropped::Closed => {
                        tracing::error!("audit outbox closed; dropping inspected record");
                    }
                }
                self.metrics.record_dropped();
            }
        }
    }
}

/// Why an inspected record could not be enqueued — both are logged and counted,
/// never silent.
enum Dropped {
    /// The outbox was full and the policy is [`FullPolicy::Shed`].
    Shed,
    /// The outbox channel is closed (the draining task has stopped).
    Closed,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use agate_audit::application::common::ports::AuditMetrics;
    use agate_proxy::domain::inspection::{LifecyclePhase, RunId, SessionId};
    use tokio::sync::mpsc;
    use uuid::Uuid;

    use super::{AgentEvent, AuditLogSink, AuditSink, FullPolicy, InspectionContext, Verdict};

    #[derive(Default)]
    struct CountingMetrics {
        dropped: AtomicUsize,
        max_depth: AtomicUsize,
    }

    impl AuditMetrics for CountingMetrics {
        fn record_appended(&self) {}
        fn record_dropped(&self) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
        fn observe_outbox_depth(&self, used: usize, _capacity: usize) {
            self.max_depth.fetch_max(used, Ordering::SeqCst);
        }
    }

    fn context() -> InspectionContext {
        InspectionContext::new(SessionId::new(Uuid::nil()), RunId::new(Uuid::nil()))
    }

    #[tokio::test]
    async fn record_enqueues_the_encoded_event() {
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(4);
        let sink = AuditLogSink::new(tx, Arc::new(CountingMetrics::default()), FullPolicy::Block);

        let event = AgentEvent::Lifecycle(LifecyclePhase::RunStarted);
        sink.record(&context(), &event, &Verdict::Allow).await;

        let bytes = rx.try_recv().expect("a record was enqueued");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("valid JSON");
        assert_eq!(json["verdict"], "allow");
    }

    #[tokio::test]
    async fn record_on_a_closed_outbox_counts_a_drop() {
        let (tx, rx) = mpsc::channel::<Vec<u8>>(1);
        drop(rx); // the outbox task has stopped

        let metrics = Arc::new(CountingMetrics::default());
        let sink = AuditLogSink::new(tx, metrics.clone(), FullPolicy::Block);
        let event = AgentEvent::Lifecycle(LifecyclePhase::RunFinished);
        // Records a drop through the port rather than panicking.
        sink.record(&context(), &event, &Verdict::Allow).await;

        assert_eq!(metrics.dropped.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn block_policy_applies_backpressure_until_a_slot_frees() {
        // Capacity 1, pre-filled: the next record cannot be enqueued yet. Block
        // must wait for a free slot rather than drop — the whole point of Block.
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(1);
        tx.send(b"first".to_vec())
            .await
            .expect("first fills capacity");

        let metrics = Arc::new(CountingMetrics::default());
        let sink = AuditLogSink::new(tx, metrics.clone(), FullPolicy::Block);
        let recording = tokio::spawn(async move {
            let event = AgentEvent::Lifecycle(LifecyclePhase::RunFinished);
            sink.record(&context(), &event, &Verdict::Allow).await;
        });

        // Give the task time to reach the blocked send; it must still be pending
        // (backpressure) and must not have dropped the record.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            !recording.is_finished(),
            "Block must wait while the outbox is full, not return"
        );
        assert_eq!(metrics.dropped.load(Ordering::SeqCst), 0, "nothing dropped");

        // Free a slot; the blocked record now enqueues and the task completes.
        assert_eq!(rx.recv().await.expect("first record"), b"first".to_vec());
        recording.await.expect("record completes once a slot frees");
        let enqueued = rx.recv().await.expect("the blocked record enqueued");
        assert_ne!(enqueued, b"first".to_vec(), "it is the awaited record");
        assert_eq!(metrics.dropped.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn shed_policy_drops_loudly_on_a_full_outbox_instead_of_blocking() {
        // Capacity 1, pre-filled: the next record cannot be enqueued.
        let (tx, _rx) = mpsc::channel::<Vec<u8>>(1);
        tx.send(b"first".to_vec()).await.expect("first fits");

        let metrics = Arc::new(CountingMetrics::default());
        let sink = AuditLogSink::new(tx, metrics.clone(), FullPolicy::Shed);
        let event = AgentEvent::Lifecycle(LifecyclePhase::RunFinished);
        // Returns at once (no blocking) and counts the shed record.
        sink.record(&context(), &event, &Verdict::Allow).await;

        assert_eq!(metrics.dropped.load(Ordering::SeqCst), 1);
        // The depth gauge saw the full channel (1 of 1 used).
        assert_eq!(metrics.max_depth.load(Ordering::SeqCst), 1);
    }
}
