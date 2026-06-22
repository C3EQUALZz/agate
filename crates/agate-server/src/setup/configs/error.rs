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

#[cfg(test)]
mod tests {
    use super::ConfigError;

    #[test]
    fn displays_each_variant() {
        assert_eq!(
            ConfigError::Required {
                key: "audit.database_url",
                hint: ""
            }
            .to_string(),
            "audit.database_url is required"
        );
        assert_eq!(
            ConfigError::Required {
                key: "policy.cel.policy_path",
                hint: "when backend = \"cel\""
            }
            .to_string(),
            "policy.cel.policy_path is required when backend = \"cel\""
        );
        assert_eq!(
            ConfigError::MustBePositive {
                key: "proxy.max_frame_bytes"
            }
            .to_string(),
            "proxy.max_frame_bytes must be greater than 0"
        );
        assert_eq!(
            ConfigError::Requires {
                key: "proxy.rate_limit_burst",
                requires: "proxy.rate_limit_per_second > 0"
            }
            .to_string(),
            "proxy.rate_limit_burst requires proxy.rate_limit_per_second > 0"
        );
        assert!(
            ConfigError::FeatureMissing {
                backend: "cel",
                feature: "policy-cel"
            }
            .to_string()
            .contains("`policy-cel` feature")
        );
    }
}
