//! OTLP trace export — the third observability pillar beside logs and metrics.
//!
//! When `[observability.tracing].enabled`, an OTLP (gRPC) span exporter feeds a
//! batch [`SdkTracerProvider`]; the provider is returned so the composition root
//! can flush it on shutdown. The matching `tracing` layer is added to the
//! subscriber in [`init_logging`](super::logging::init_logging).

use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;

use crate::setup::configs::TracingConfig;

/// The tracer name spans are recorded under.
pub const TRACER_NAME: &str = "agate-server";

/// Build an OTLP batch trace pipeline when enabled, returning the provider to
/// keep for shutdown. `None` when tracing export is off (spans stay log-only).
#[must_use]
pub fn build_tracer_provider(config: &TracingConfig) -> Option<SdkTracerProvider> {
    if !config.enabled {
        return None;
    }

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(config.endpoint.clone())
        .build()
        .expect("build the OTLP span exporter");

    let resource = Resource::builder()
        .with_service_name(config.service_name.clone())
        .build();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    Some(provider)
}

#[cfg(test)]
mod tests {
    use super::build_tracer_provider;
    use crate::setup::configs::TracingConfig;

    #[test]
    fn disabled_tracing_builds_no_provider() {
        let config = TracingConfig {
            enabled: false,
            ..TracingConfig::default()
        };
        assert!(build_tracer_provider(&config).is_none());
    }

    // The exporter connects lazily, so building a provider against an
    // unreachable endpoint succeeds without contacting it; shut it down so the
    // batch exporter's background task does not outlive the test.
    #[tokio::test]
    async fn enabled_tracing_builds_a_provider() {
        let config = TracingConfig {
            enabled: true,
            ..TracingConfig::default()
        };
        let provider = build_tracer_provider(&config).expect("a provider when enabled");
        provider.shutdown().expect("shut the provider down");
    }
}
