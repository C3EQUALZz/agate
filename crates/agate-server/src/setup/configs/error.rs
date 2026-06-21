use std::fmt;

/// A static configuration error — a misconfiguration caught at startup, before
/// any I/O, so a bad config aborts fast rather than running degraded. Each
/// variant names the offending fully-qualified key (`section.key`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// A required key was absent or blank.
    Required {
        key: &'static str,
        /// Where to set it (e.g. the TOML key and its env override); may be empty.
        hint: &'static str,
    },
    /// A numeric setting must be greater than zero.
    MustBePositive { key: &'static str },
    /// One setting only makes sense when another is also set.
    Requires {
        key: &'static str,
        requires: &'static str,
    },
    /// A backend was selected without the Cargo feature that provides it.
    FeatureMissing {
        backend: &'static str,
        feature: &'static str,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Required { key, hint: "" } => write!(f, "{key} is required"),
            ConfigError::Required { key, hint } => write!(f, "{key} is required {hint}"),
            ConfigError::MustBePositive { key } => write!(f, "{key} must be greater than 0"),
            ConfigError::Requires { key, requires } => write!(f, "{key} requires {requires}"),
            ConfigError::FeatureMissing { backend, feature } => write!(
                f,
                "policy.backend = \"{backend}\" requires building agate-server with the \
                 `{feature}` feature"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}
