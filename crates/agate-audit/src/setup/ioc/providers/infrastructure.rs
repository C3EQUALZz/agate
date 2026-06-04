//! Infrastructure providers: process-wide singletons (App scope) and the
//! per-request transaction and gateways (Request scope).
//!
//! Named `async fn` instantiators (returning [`InstantiatorResult`]) keep the
//! trait-object coercions readable; App-scope constructors are pure and have no
//! `await`, so they live in a sync sub-registry that `froodi` merges into the
//! async container.

use std::sync::Arc;

use froodi::{
    DefaultScope::{App, Request},
    Inject, InstantiatorResult,
    async_impl::RegistryWithSync,
    async_registry, instance, registry,
};
use sqlx::PgPool;

use agate_crypto::{CryptoRegistry, HashAlgo, Hasher};

use crate::application::common::ports::TransactionManager;
use crate::domain::merkle::{MerkleHasher, TransparencyLogFactory};
use crate::infrastructure::persistence::log::postgres::{
    PostgresLogCommandGateway, PostgresLogQueryGateway,
};
use crate::infrastructure::persistence::postgres::{PgTransactionManager, TxSlot};
use crate::infrastructure::{SystemClock, UuidLogIdGenerator};

/// Adapters and the request transaction. `pool` becomes the App-scope singleton
/// every request-scoped gateway/transaction borrows from.
pub(crate) fn infrastructure_providers(pool: PgPool) -> RegistryWithSync {
    async_registry! {
        scope(Request) [
            provide(provide_tx_slot),
            provide(provide_transaction_manager, finalizer = rollback_open_transaction),
            provide(provide_command_gateway),
            provide(provide_query_gateway),
        ],
        extend(registry! {
            scope(App) [
                provide(instance(pool)),
                provide(|| Ok(UuidLogIdGenerator)),
                provide(|| Ok(SystemClock)),
                provide(|| Ok(TransparencyLogFactory::new(default_hasher()))),
                provide(|| Ok(MerkleHasher::new(default_hasher()))),
            ]
        }),
    }
}

/// One empty transaction slot per request; the manager fills it on `begin`.
async fn provide_tx_slot() -> InstantiatorResult<TxSlot> {
    Ok(TxSlot::new(None))
}

/// Owns the request's commit boundary; shares the slot with the command gateway.
async fn provide_transaction_manager(
    Inject(pool): Inject<PgPool>,
    Inject(slot): Inject<TxSlot>,
) -> InstantiatorResult<PgTransactionManager> {
    Ok(PgTransactionManager::new((*pool).clone(), slot))
}

/// Write side: runs on the shared request transaction (never commits).
async fn provide_command_gateway(
    Inject(slot): Inject<TxSlot>,
    Inject(factory): Inject<TransparencyLogFactory>,
) -> InstantiatorResult<PostgresLogCommandGateway> {
    Ok(PostgresLogCommandGateway::new(slot, (*factory).clone()))
}

/// Read side: queries the pool directly and rebuilds proofs with the hasher.
async fn provide_query_gateway(
    Inject(pool): Inject<PgPool>,
    Inject(hasher): Inject<MerkleHasher>,
) -> InstantiatorResult<PostgresLogQueryGateway> {
    Ok(PostgresLogQueryGateway::new(
        (*pool).clone(),
        (*hasher).clone(),
    ))
}

/// Safety net: roll back a transaction left open when a request scope closes.
async fn rollback_open_transaction(manager: Arc<PgTransactionManager>) {
    let _ = manager.rollback().await;
}

fn default_hasher() -> Arc<dyn Hasher> {
    CryptoRegistry::hasher(HashAlgo::Sha256).expect("SHA-256 is always available")
}
