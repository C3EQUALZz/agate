//! The real [`ProxyMetrics`] adapter: emits counters through the `metrics`
//! facade (a no-op until a recorder is installed at the composition root).

use metrics::counter;

use crate::application::common::ports::{InspectionOutcome, ProxyMetrics};

/// Emits the proxy data-plane counters via the `metrics` facade.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProxyMetricsRecorder;

impl ProxyMetrics for ProxyMetricsRecorder {
    fn record_run(&self) {
        counter!("agate_runs_total").increment(1);
    }

    fn record_upstream_error(&self) {
        counter!("agate_upstream_errors_total").increment(1);
    }

    fn record_inspected(&self, outcome: InspectionOutcome) {
        counter!("agate_events_inspected_total", "outcome" => outcome.label()).increment(1);
    }
}
