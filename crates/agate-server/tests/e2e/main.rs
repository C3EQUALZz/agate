//! End-to-end tests for the composition root: boot the proxy in front of a stub
//! AG-UI agent, backed by a real PostgreSQL audit store, and assert that a run
//! driven through the proxy is recorded to the transparency log. Wired as an
//! explicit `[[test]]` target.

// The plugin-engine e2e is compiled only when both engines are built in (CI's
// `cargo test --all-features` covers it); the default test build skips it.
mod controls;
#[cfg(all(feature = "policy-cel", feature = "policy-rego"))]
mod engines;
mod fixture;
mod policy;
mod server;
mod stress;
