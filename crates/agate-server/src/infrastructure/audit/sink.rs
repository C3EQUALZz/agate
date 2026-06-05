use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

use agate_proxy::application::common::ports::AuditSink;
use agate_proxy::application::inspection::InspectionContext;
use agate_proxy::domain::inspection::{AgentEvent, Verdict};

use super::record::encode_record;

/// The proxy-side [`AuditSink`]: encodes each inspected event and enqueues it on
/// the outbox channel, returning at once so the forwarding path is never blocked
/// on the audit write. A full channel applies backpressure (the send awaits);
/// it never silently drops a record.
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
        }
    }
}
