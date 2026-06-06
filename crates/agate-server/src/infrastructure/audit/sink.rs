use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

use agate_proxy::application::common::ports::AuditSink;
use agate_proxy::application::inspection::InspectionContext;
use agate_proxy::domain::inspection::{AgentEvent, Verdict};

use super::record::encode_record;

/// The proxy-side [`AuditSink`]: encodes each inspected event and enqueues it on
/// the outbox channel, returning at once so the forwarding path is never blocked
/// on the audit write. A full channel applies backpressure (the send awaits).
///
/// A record is dropped only if the channel is closed — which happens at
/// shutdown, once the outbox task has stopped. The drop is logged, but since
/// `record` returns `()` (an outbox contract), the caller cannot observe it.
pub struct AuditLogSink {
    tx: Sender<Vec<u8>>,
}

impl AuditLogSink {
    #[must_use]
    pub fn new(tx: Sender<Vec<u8>>) -> Self {
        Self { tx }
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
        if self.tx.send(record).await.is_err() {
            tracing::error!("audit outbox closed; dropping inspected record");
            metrics::counter!("agate_audit_records_dropped_total").increment(1);
        } else {
            tracing::debug!(run = %context.run.0, "enqueued inspected event to the audit outbox");
        }
    }
}

#[cfg(test)]
mod tests {
    use agate_proxy::domain::inspection::{LifecyclePhase, RunId, SessionId};
    use tokio::sync::mpsc;
    use uuid::Uuid;

    use super::{AgentEvent, AuditLogSink, AuditSink, InspectionContext, Verdict};

    fn context() -> InspectionContext {
        InspectionContext::new(SessionId(Uuid::nil()), RunId(Uuid::nil()))
    }

    #[tokio::test]
    async fn record_enqueues_the_encoded_event() {
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(4);
        let sink = AuditLogSink::new(tx);

        let event = AgentEvent::Lifecycle(LifecyclePhase::RunStarted);
        sink.record(&context(), &event, &Verdict::Allow).await;

        let bytes = rx.try_recv().expect("a record was enqueued");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("valid JSON");
        assert_eq!(json["verdict"], "allow");
    }

    #[tokio::test]
    async fn record_on_a_closed_outbox_does_not_panic() {
        let (tx, rx) = mpsc::channel::<Vec<u8>>(1);
        drop(rx); // the outbox task has stopped

        let sink = AuditLogSink::new(tx);
        let event = AgentEvent::Lifecycle(LifecyclePhase::RunFinished);
        // Logs + counts a drop rather than panicking.
        sink.record(&context(), &event, &Verdict::Allow).await;
    }
}
