//! Shared HTTP concerns for v1: error mapping and request dispatch.

pub mod dispatch;
pub mod error;

pub use dispatch::dispatcher;
pub use error::HttpError;
