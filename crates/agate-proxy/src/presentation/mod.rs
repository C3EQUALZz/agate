//! Presentation layer: the reverse-proxy data path. [`inspect_stream`] streams
//! the agent's SSE response through inspection; [`http`] exposes it as an axum
//! reverse-proxy handler.

pub mod http;
pub mod stream;

pub use stream::inspect_stream;
