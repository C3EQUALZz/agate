use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

use crate::setup::configs::{LogFormat, LoggingConfig};

/// Install the global tracing subscriber per config.
///
/// A disabled config installs nothing — logs are silenced. The filter is taken
/// from `RUST_LOG` when set, else the configured `level`, else `info`. The
/// format selects the console (`pretty`) or structured (`json`) layer.
pub fn init_logging(config: &LoggingConfig) {
    if !config.enabled {
        return;
    }

    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&config.level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = tracing_subscriber::registry().with(filter);
    match config.format {
        LogFormat::Pretty => registry.with(tracing_subscriber::fmt::layer()).init(),
        LogFormat::Json => registry
            .with(tracing_subscriber::fmt::layer().json())
            .init(),
    }
}
