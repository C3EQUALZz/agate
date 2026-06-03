//! Composition root: wires use cases to adapters with the `froodi` IoC
//! container and routes them with the messaging [`Registry`].
//!
//! Providers are split into modular registries (the `froodi` idiom) and merged
//! here:
//! - [`providers`] — infrastructure: App-scope singletons (pool, id generator,
//!   clock, hashing) and the Request-scope transaction and gateways.
//! - [`handlers`] — application: the use-case handlers and pipeline behavior.
//!
//! Per request, open a child scope with `container.clone().enter_build()`; the
//! command gateway and [`PgTransactionManager`] share that scope's [`TxSlot`],
//! and a finalizer rolls back any transaction still open when it closes.
//!
//! The [`Registry`] maps request types to handlers/behaviors and the
//! [`Dispatcher`](crate::application::common::messaging::Dispatcher) resolves
//! them through the [`Resolve`] bridge implemented here for the container.

mod handlers;
mod providers;

use std::sync::Arc;

use async_trait::async_trait;
use froodi::{DefaultScope::App, async_impl::Container, async_registry};
use sqlx::PgPool;

use crate::application::common::behaviors::TransactionBehavior;
use crate::application::common::messaging::{Registry, Resolve, ResolveError};
use crate::application::usecases::append_record::{AppendRecord, AppendRecordHandler};
use crate::application::usecases::create_log::{CreateLog, CreateLogHandler};
use crate::application::usecases::get_consistency_proof::{
    GetConsistencyProof, GetConsistencyProofHandler,
};
use crate::application::usecases::get_inclusion_proof::{
    GetInclusionProof, GetInclusionProofHandler,
};

/// Bridges the container-agnostic messaging layer to `froodi`: `Resolve<T>`
/// just delegates to `Container::get::<T>()`.
#[async_trait]
impl<T: Send + Sync + 'static> Resolve<T> for Container {
    async fn resolve(&self) -> Result<Arc<T>, ResolveError> {
        self.get::<T>()
            .await
            .map_err(|error| ResolveError(error.to_string()))
    }
}

/// The routing table: which handler and behaviors each request type uses.
/// Commands run inside the transaction behavior; read-only queries do not.
#[must_use]
pub fn build_registry() -> Registry<Container> {
    let mut registry = Registry::new();
    registry.handler::<CreateLog, CreateLogHandler>();
    registry.handler::<AppendRecord, AppendRecordHandler>();
    registry.handler::<GetInclusionProof, GetInclusionProofHandler>();
    registry.handler::<GetConsistencyProof, GetConsistencyProofHandler>();
    registry.behavior::<CreateLog, TransactionBehavior>();
    registry.behavior::<AppendRecord, TransactionBehavior>();
    registry
}

/// Builds the IoC container, started at the App scope. Open a Request scope per
/// request with `container.clone().enter_build()`.
#[must_use]
pub fn build_container(pool: PgPool) -> Container {
    let ioc = async_registry! {
        extend(
            providers::infrastructure_providers(pool),
            handlers::handler_providers(),
        )
    };
    Container::new_with_start_scope(ioc, App)
}
