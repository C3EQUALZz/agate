//! HTTP presentation: an axum API over the audit use cases.
//!
//! Each HTTP request runs in one `froodi` request scope — hence one
//! transaction for commands — installed by the `froodi::axum` layer. Handlers
//! pull that request-scope container out of the request and dispatch through
//! the messaging [`Dispatcher`](crate::application::common::messaging::Dispatcher),
//! so the same pipeline (incl. the transaction behavior) runs per request.

mod dto;
mod error;
mod handlers;
mod router;

pub use router::build_app;
