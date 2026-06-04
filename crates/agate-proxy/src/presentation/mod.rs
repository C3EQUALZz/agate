//! Presentation layer: the reverse-proxy data path. [`inspect_stream`] is the
//! core — it streams the agent's SSE response through inspection — and the axum
//! HTTP handler (added next) wires it to the transport.

pub mod stream;

pub use stream::inspect_stream;
