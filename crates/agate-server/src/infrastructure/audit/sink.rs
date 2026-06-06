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
        } else {
            tracing::debug!(run = %context.run.0, "enqueued inspected event to the audit outbox");
        }
    }
}
