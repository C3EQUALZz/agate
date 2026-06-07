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

    // `try_init` (not `init`) so the install is idempotent: a second call — only
    // reachable from tests sharing a process — is a no-op rather than a panic.
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(otel_layer)
        .try_init();

    provider
}

#[cfg(test)]
mod tests {
    use super::init_logging;
    use crate::setup::configs::{LogFormat, LoggingConfig, TracingConfig};

    #[test]
    fn disabled_logging_installs_nothing() {
        let logging = LoggingConfig {
            enabled: false,
            ..LoggingConfig::default()
        };
        assert!(init_logging(&logging, &TracingConfig::default()).is_none());
    }

    #[test]
    fn enabled_logging_without_tracing_returns_no_provider() {
        let logging = LoggingConfig {
            enabled: true,
            format: LogFormat::Pretty,
            ..LoggingConfig::default()
        };
        let tracing_config = TracingConfig {
            enabled: false,
            ..TracingConfig::default()
        };
        assert!(init_logging(&logging, &tracing_config).is_none());
    }

    // JSON format + tracing on, so the JSON arm and the OTLP layer are built;
    // `init_logging` is idempotent (`try_init`), so running alongside the other
    // tests is fine. Shut the returned provider down to stop its export task.
    #[tokio::test]
    async fn enabled_logging_with_tracing_returns_a_provider() {
        let logging = LoggingConfig {
            enabled: true,
            format: LogFormat::Json,
            ..LoggingConfig::default()
        };
        let tracing_config = TracingConfig {
            enabled: true,
            ..TracingConfig::default()
        };
        let provider = init_logging(&logging, &tracing_config).expect("a provider when enabled");
        provider.shutdown().expect("shut the provider down");
    }
}
