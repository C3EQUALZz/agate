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
    let Some(addr) = endpoint(config) else {
        return false;
    };
    PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()
        .expect("install the Prometheus metrics exporter");
    true
}

/// The address the exporter should listen on, or `None` when metrics are
/// disabled. Pure decision + parse, split out from the side-effecting install
/// so it can be tested directly.
fn endpoint(config: &MetricsConfig) -> Option<SocketAddr> {
    if !config.enabled || config.exporter == MetricsExporter::None {
        return None;
    }
    let addr = config.bind.parse().unwrap_or_else(|error| {
        panic!(
            "invalid observability.metrics.bind '{}': {error}",
            config.bind
        )
    });
    Some(addr)
}

#[cfg(test)]
mod tests {
    use metrics_exporter_prometheus::PrometheusBuilder;

    use super::endpoint;
    use crate::setup::configs::{MetricsConfig, MetricsExporter};

    fn config(enabled: bool, exporter: MetricsExporter, bind: &str) -> MetricsConfig {
        MetricsConfig {
            enabled,
            exporter,
            bind: bind.into(),
        }
    }

    #[test]
    fn endpoint_is_none_when_disabled() {
        let c = config(false, MetricsExporter::Prometheus, "0.0.0.0:9090");
        assert!(endpoint(&c).is_none());
    }

    #[test]
    fn endpoint_is_none_when_exporter_is_none() {
        let c = config(true, MetricsExporter::None, "0.0.0.0:9090");
        assert!(endpoint(&c).is_none());
    }

    #[test]
    fn endpoint_parses_the_bind_when_enabled() {
        let c = config(true, MetricsExporter::Prometheus, "127.0.0.1:9100");
        assert_eq!(
            endpoint(&c).expect("some addr").to_string(),
            "127.0.0.1:9100"
        );
    }

    #[test]
    #[should_panic(expected = "invalid observability.metrics.bind")]
    fn endpoint_panics_on_an_invalid_bind() {
        let c = config(true, MetricsExporter::Prometheus, "not-an-address");
        let _ = endpoint(&c);
    }

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
