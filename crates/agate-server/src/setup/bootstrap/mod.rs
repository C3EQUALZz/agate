//! Bootstrap: assemble the running server from its parts.

pub mod appender;
pub mod server;

pub use appender::ScopedAppender;
pub use server::{Server, build_server};
