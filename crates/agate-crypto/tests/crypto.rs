//! Integration suite for agate-crypto: one binary, modules per subdomain,
//! shared helpers in `common`. Backends are feature-gated, so the
//! signature/encryption modules only compile their cases when enabled (run
//! `cargo test -p agate-crypto --all-features` to exercise GOST and Ed25519).

mod common;
mod encryption;
mod hashing;
#[cfg(feature = "ed25519")]
mod signing;
