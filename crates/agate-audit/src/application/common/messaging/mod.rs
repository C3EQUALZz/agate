//! Mediator / CQRS messaging primitives: a [`Request`] is handled by one
//! [`RequestHandler`], wrapped by an ordered list of [`Behavior`]s
//! (chain of responsibility). Behaviors are composed at the composition root,
//! conditionally by config (e.g. enable metrics/tracing via TOML).

pub mod behavior;
pub mod handler;
pub mod mediator;
pub mod request;

pub use behavior::{Behavior, BoxFuture, Next};
pub use handler::RequestHandler;
pub use mediator::Mediator;
pub use request::{Command, Query, Request};
