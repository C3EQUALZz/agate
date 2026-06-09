use serde::{Deserialize, Serialize};

/// `[tls]` — optional TLS termination at the server's listener.
///
/// Off by default: Agate then serves plain HTTP, which is sensible only behind a
/// TLS-terminating gateway. Set `enabled` with a PEM `cert` chain + `key` to
/// serve HTTPS directly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TlsConfig {
    /// Serve HTTPS instead of plain HTTP. When `false`, `cert`/`key` are ignored.
    pub enabled: bool,
    /// Path to the PEM certificate chain (leaf certificate first).
    pub cert: String,
    /// Path to the PEM private key for `cert`.
    pub key: String,
}

impl TlsConfig {
    /// Fail fast on enabling TLS without the certificate material.
    pub fn validate(&self) -> Result<(), String> {
        if self.enabled && (self.cert.trim().is_empty() || self.key.trim().is_empty()) {
            return Err(
                "tls.cert and tls.key are required when tls.enabled (paths to the PEM \
                 certificate chain and private key)"
                    .into(),
            );
        }
        Ok(())
    }
}
