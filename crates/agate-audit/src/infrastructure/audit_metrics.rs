//! The real [`AuditMetrics`] adapter: emits counters through the `metrics`
//! facade. A no-op until a recorder is installed at the composition root, so it
//! is always safe to wire in.

use metrics::{counter, gauge};

use crate::application::common::ports::AuditMetrics;

/// Emits the audit append/drop counters and the outbox-depth gauge via the
/// `metrics` facade.
#[derive(Debug, Default, Clone, Copy)]
pub struct AuditMetricsRecorder;

impl AuditMetrics for AuditMetricsRecorder {
    fn record_appended(&self) {
        counter!("agate_audit_records_appended_total").increment(1);
    }

    fn record_dropped(&self) {
        counter!("agate_audit_records_dropped_total").increment(1);
    }

    fn observe_outbox_depth(&self, used: usize, capacity: usize) {
        gauge!("agate_audit_outbox_depth").set(gauge_value(used));
        gauge!("agate_audit_outbox_capacity").set(gauge_value(capacity));
    }
}

/// Convert a count to a gauge value without precision loss: queue depths are
/// small, so a `u32`→`f64` (exact) conversion is enough; an absurd value
/// saturates rather than rounding silently.
fn gauge_value(count: usize) -> f64 {
    u32::try_from(count).map_or(f64::from(u32::MAX), f64::from)
}
