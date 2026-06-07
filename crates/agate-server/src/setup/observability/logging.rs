use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

use super::tracing::{TRACER_NAME, build_tracer_provider};
use crate::setup::configs::{LogFormat, LoggingConfig, TracingConfig};

/// Install the global tracing subscriber per config: the log layer (console or
/// JSON) plus, when `[observability.tracing]` is enabled, an OTLP trace layer.
///
/// A disabled logging config installs nothing — logs are silenced and no traces
/// are exported. The filter is `RUST_LOG` when set, else the configured `level`,
/// else `info`. Returns the OTLP [`SdkTracerProvider`] (when tracing is on) so
/// the caller can flush it on shutdown.
#[must_use]
pub fn init_logging(
    logging: &LoggingConfig,
    tracing_config: &TracingConfig,
) -> Option<SdkTracerProvider> {
    if !logging.enabled {
        return None;
    }

    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&logging.level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = match logging.format {
        LogFormat::Pretty => tracing_subscriber::fmt::layer().boxed(),
        LogFormat::Json => tracing_subscriber::fmt::layer().json().boxed(),
    };

    let provider = build_tracer_provider(tracing_config);
    let otel_layer = provider
        .as_ref()
        .map(|provider| tracing_opentelemetry::layer().with_tracer(provider.tracer(TRACER_NAME)));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(otel_layer)
        .init();

    provider
}
