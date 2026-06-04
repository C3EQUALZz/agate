use async_trait::async_trait;

use crate::application::common::ports::AuditSink;
use crate::application::inspection::InspectionContext;
use crate::domain::inspection::{AgentEvent, Verdict};

/// Audit sink that drops every record — the default until the audit context is
/// wired in (at the top-level server).
pub struct NoopAuditSink;

#[async_trait]
impl AuditSink for NoopAuditSink {
    async fn record(&self, _: &InspectionContext, _: &AgentEvent, _: &Verdict<AgentEvent>) {}
}
