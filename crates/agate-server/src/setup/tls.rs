//! TLS termination: build a rustls server config from the configured PEM files.
//!
//! Used by the composition root to serve HTTPS when `[tls].enabled`. The crypto
//! provider is pinned to ring (already in the dependency tree via reqwest), so
//! `RustlsConfig` construction never hits the "no process-default provider"
//! panic.

use axum_server::tls_rustls::RustlsConfig;

use crate::setup::configs::TlsConfig;

/// Load a rustls server config from the configured PEM certificate + key.
///
/// Installs the process-wide crypto provider first. Panics on an unreadable or
/// invalid certificate/key — a startup misconfiguration, surfaced fast.
pub async fn load_tls(config: &TlsConfig) -> RustlsConfig {
    install_crypto_provider();
    RustlsConfig::from_pem_file(&config.cert, &config.key)
        .await
        .expect("load the TLS certificate and private key")
}

/// Install ring as the process default rustls `CryptoProvider`. Idempotent: a
/// second call (or one after reqwest installed its own) is a no-op.
fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::load_tls;
    use crate::setup::configs::TlsConfig;

    #[tokio::test]
    async fn loads_a_self_signed_cert_and_key() {
        // Generate a throwaway cert at test time (never commit key material —
        // the secret scanner would flag it).
        let generated = rcgen::generate_simple_self_signed(["localhost".to_owned()])
            .expect("generate a self-signed certificate");
        let mut cert_file = tempfile::NamedTempFile::new().expect("a temp cert file");
        let mut key_file = tempfile::NamedTempFile::new().expect("a temp key file");
        cert_file
            .write_all(generated.cert.pem().as_bytes())
            .expect("write the cert");
        key_file
            .write_all(generated.key_pair.serialize_pem().as_bytes())
            .expect("write the key");

        let config = TlsConfig {
            enabled: true,
            cert: cert_file.path().to_string_lossy().into_owned(),
            key: key_file.path().to_string_lossy().into_owned(),
        };

        // Builds without panicking: the crypto provider installs and the PEM
        // parses into a usable server config.
        let _config = load_tls(&config).await;
    }
}
