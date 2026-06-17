//! Bootstrap: assemble the running server from its parts.

pub mod appender;
pub mod checkpoint_issuer;
/// Shared scope-per-command audit dispatch behind the outbox and scheduler.
pub mod scope;
pub mod server;

pub use appender::ScopedAppender;
pub use checkpoint_issuer::ScopedIssuer;
pub use server::{CheckpointSettings, OutboxSettings, Server, ServerConfig, build_server};
