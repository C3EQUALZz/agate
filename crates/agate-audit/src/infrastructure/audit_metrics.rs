//! The real [`AuditMetrics`] adapter: emits counters through the `metrics`
//! facade. A no-op until a recorder is installed at the composition root, so it
//! is always safe to wire in.

use metrics::counter;

use crate::application::common::ports::AuditMetrics;

/// Emits the audit append/drop counters via the `metrics` facade.
#[derive(Debug, Default, Clone, Copy)]
pub struct AuditMetricsRecorder;

impl AuditMetrics for AuditMetricsRecorder {
    fn record_appended(&self) {
        counter!("agate_audit_records_appended_total").increment(1);
    }

    fn record_dropped(&self) {
        counter!("agate_audit_records_dropped_total").increment(1);
    }
}
