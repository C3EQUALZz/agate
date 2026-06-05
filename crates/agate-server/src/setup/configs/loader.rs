use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};

use super::app_config::AppConfig;

/// Where the config file is read from when `AGATE_CONFIG` is unset.
const DEFAULT_PATH: &str = "/etc/agate/agate.toml";

/// Load the configuration by layering, lowest precedence first:
/// 1. built-in defaults,
/// 2. the TOML file at `AGATE_CONFIG` (default `/etc/agate/agate.toml`; a missing
///    file is simply skipped),
/// 3. environment variables `AGATE__SECTION__KEY` (so secrets and per-deployment
///    overrides need no file edit).
pub fn load() -> Result<AppConfig, Box<figment::Error>> {
    let path = std::env::var("AGATE_CONFIG").unwrap_or_else(|_| DEFAULT_PATH.to_owned());
    figment(Toml::file(path)).extract().map_err(Box::new)
}

/// The layered figment, parameterized over the file provider so tests can supply
/// an in-memory document instead of a path.
fn figment<P: figment::Provider>(file: P) -> Figment {
    Figment::from(Serialized::defaults(AppConfig::default()))
        .merge(file)
        .merge(Env::prefixed("AGATE__").split("__"))
}

#[cfg(test)]
// Jail's closure must return `Result<(), figment::Error>`, whose Err is large.
#[allow(clippy::result_large_err)]
mod tests {
    use figment::Jail;
    use figment::providers::{Format, Toml};

    use super::super::observability::LogFormat;
    use super::figment;

    #[test]
    fn file_values_override_defaults_and_env_overrides_the_file() {
        Jail::expect_with(|jail| {
            jail.create_file(
                "agate.toml",
                r#"
                    [proxy]
                    agent_endpoint = "http://from-file/run"

                    [observability.logging]
                    format = "json"
                "#,
            )?;
            jail.set_env("AGATE__PROXY__AGENT_ENDPOINT", "http://from-env/run");

            let config: super::AppConfig = figment(Toml::file("agate.toml"))
                .extract()
                .map_err(|e| e.to_string())?;

            // env wins over the file
            assert_eq!(config.proxy.agent_endpoint, "http://from-env/run");
            // file wins over the default
            assert_eq!(config.observability.logging.format, LogFormat::Json);
            // untouched value keeps its default
            assert_eq!(config.proxy.bind, "0.0.0.0:8080");
            Ok(())
        });
    }

    #[test]
    fn a_missing_file_falls_back_to_defaults() {
        Jail::expect_with(|_jail| {
            let config: super::AppConfig = figment(Toml::file("does-not-exist.toml"))
                .extract()
                .map_err(|e| e.to_string())?;
            assert!(config.observability.logging.enabled);
            assert_eq!(config.proxy.bind, "0.0.0.0:8080");
            Ok(())
        });
    }
}
