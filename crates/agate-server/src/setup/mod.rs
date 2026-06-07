//! Composition root: typed configuration, observability, and the bootstrap that
//! wires the proxy data plane to the audit transparency log.

pub mod bootstrap;
pub mod configs;
pub mod observability;
pub mod tls;
