use async_trait::async_trait;

use crate::application::inspection::InspectionContext;
use crate::domain::inspection::{AgentEvent, Verdict};

/// Records an inspected event and its verdict for the audit trail. Modeled as an
/// async outbox: implementations enqueue and return quickly so the forwarding
/// path is never blocked; durability and log ordering are the adapter's concern.
#[async_trait]
pub trait AuditSink: Send + Sync {
    async fn record(
        &self,
        context: &InspectionContext,
        event: &AgentEvent,
        verdict: &Verdict<AgentEvent>,
    );
}
