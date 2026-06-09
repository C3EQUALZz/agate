//! Providers, split by concern and merged by the container:
//! - [`agnostic`] — backend-agnostic App-scope singletons.
//! - [`postgres`] — the PostgreSQL backend: pool, transaction, gateways, anchor.
//! - [`handlers`] — the use-case handlers and the pipeline behavior.
//! - [`dispatch`] — the routing table and the per-request dispatcher.

pub mod agnostic;
pub mod dispatch;
pub mod handlers;
pub mod postgres;

pub(crate) use agnostic::agnostic_providers;
pub(crate) use dispatch::dispatch_providers;
pub(crate) use handlers::handler_providers;
pub(crate) use postgres::postgres_providers;
