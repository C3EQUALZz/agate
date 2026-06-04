//! Providers, split by concern and merged by the container:
//! - [`infrastructure`] — App-scope singletons and the per-request transaction
//!   and gateways.
//! - [`handlers`] — the use-case handlers and the pipeline behavior.

pub mod handlers;
pub mod infrastructure;

pub(crate) use handlers::handler_providers;
pub(crate) use infrastructure::infrastructure_providers;
