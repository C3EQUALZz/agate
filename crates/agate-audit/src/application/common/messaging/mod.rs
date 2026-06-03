//! Mediator / CQRS messaging primitives: a [`Request`] is handled by one
//! [`RequestHandler`], wrapped by an ordered list of [`Behavior`]s
//! (chain of responsibility). A [`Registry`] maps each request type to its
//! handler and behaviors; a [`Dispatcher`] resolves them from a container
//! (via [`Resolve`]) and runs the chain — so behaviors are composed at the
//! composition root, conditionally by config (e.g. enable metrics via TOML).

pub mod behavior;
pub mod dispatcher;
pub mod handler;
pub mod mediator;
pub mod registry;
pub mod request;
pub mod resolve;

pub use behavior::{Behavior, BoxFuture, Next};
pub use dispatcher::Dispatcher;
pub use handler::RequestHandler;
pub use mediator::Mediator;
pub use registry::Registry;
pub use request::{Command, Query, Request};
pub use resolve::{Resolve, ResolveError};
