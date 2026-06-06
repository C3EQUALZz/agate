use std::net::SocketAddr;

use metrics_exporter_prometheus::PrometheusBuilder;

use crate::setup::configs::{MetricsConfig, MetricsExporter};

/// Install the metrics recorder and its exporter per config, returning whether
/// one was installed (so the caller can log the endpoint).
///
/// When disabled (or `exporter = "none"`) nothing is installed and every
/// `metrics::counter!` call elsewhere becomes a no-op — the `metrics` facade
/// does nothing without a global recorder. Must be called from within a Tokio
/// runtime: the Prometheus exporter spawns an HTTP listener serving `/metrics`
/// on its own port.
pub fn init_metrics(config: &MetricsConfig) -> bool {
    if !config.enabled || config.exporter == MetricsExporter::None {
        return false;
    }
    let addr: SocketAddr = config.bind.parse().unwrap_or_else(|error| {
        panic!(
            "invalid observability.metrics.bind '{}': {error}",
            config.bind
        )
    });
    PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()
        .expect("install the Prometheus metrics exporter");
    true
}

#[cfg(test)]
mod tests {
    use metrics_exporter_prometheus::PrometheusBuilder;

    #[test]
    fn prometheus_renders_agate_counters() {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, || {
            metrics::counter!("agate_runs_total").increment(1);
            metrics::counter!("agate_events_inspected_total", "outcome" => "deny").increment(2);
            metrics::counter!("agate_audit_records_appended_total").increment(1);
        });

        let text = handle.render();
        assert!(text.contains("agate_runs_total"));
        assert!(text.contains("agate_events_inspected_total"));
        assert!(text.contains("outcome=\"deny\""));
        assert!(text.contains("agate_audit_records_appended_total"));
    }
}
