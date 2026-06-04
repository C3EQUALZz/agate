//! Providers, split by concern and merged by the container:
//! - [`infrastructure`] — App-scope singletons and the per-request transaction
//!   and gateways.
//! - [`handlers`] — the use-case handlers and the pipeline behavior.
//! - [`dispatch`] — the routing table and the per-request dispatcher.

pub mod dispatch;
pub mod handlers;
pub mod infrastructure;

pub(crate) use dispatch::dispatch_providers;
pub(crate) use handlers::handler_providers;
pub(crate) use infrastructure::infrastructure_providers;
