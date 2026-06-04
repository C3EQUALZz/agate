use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use agate_proxy::application::common::ports::AuditSink;
use agate_proxy::application::inspection::InspectionContext;
use agate_proxy::domain::inspection::{AgentEvent, Verdict};

/// Audit sink test double that counts how many events it recorded.
#[derive(Default)]
pub struct CountingAudit {
    records: AtomicUsize,
}

impl CountingAudit {
    pub fn recorded(&self) -> usize {
        self.records.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl AuditSink for CountingAudit {
    async fn record(&self, _: &InspectionContext, _: &AgentEvent, _: &Verdict<AgentEvent>) {
        self.records.fetch_add(1, Ordering::SeqCst);
    }
}
