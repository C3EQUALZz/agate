//! PostgreSQL backend providers: the connection pool (App singleton) and the
//! per-request transaction, gateways, and checkpoint anchor (Request scope).
//!
//! Each storage port is registered as its backend-agnostic
//! [handle](crate::setup::ioc::handles): the concrete Postgres adapter is built
//! here and wrapped, so handlers inject the handle and never name Postgres.

use std::sync::Arc;

use froodi::{
    DefaultScope::{App, Request},
    Inject, InstantiatorResult,
    async_impl::RegistryWithSync,
    async_registry, instance, registry,
};
use sqlx::PgPool;

use crate::application::common::ports::{
    CheckpointAnchor, LogCommandGateway, LogQueryGateway, TransactionManager,
};
use crate::domain::merkle::{MerkleHasher, TransparencyLogFactory};
use crate::infrastructure::persistence::log::postgres::{
    PostgresCheckpointAnchor, PostgresLogCommandGateway, PostgresLogQueryGateway,
};
use crate::infrastructure::persistence::postgres::{PgTransactionManager, TxSlot};
use crate::setup::ioc::handles::{
    CheckpointAnchorHandle, LogCommandGatewayHandle, LogQueryGatewayHandle,
    TransactionManagerHandle,
};

/// The Postgres adapters and the request transaction. `pool` becomes the
/// App-scope singleton every request-scoped gateway/transaction borrows from.
pub(crate) fn postgres_providers(pool: PgPool) -> RegistryWithSync {
    async_registry! {
        scope(Request) [
            provide(provide_tx_slot),
            provide(provide_transaction_manager, finalizer = rollback_open_transaction),
            provide(provide_transaction_manager_handle),
            provide(provide_command_gateway_handle),
            provide(provide_query_gateway_handle),
            provide(provide_checkpoint_anchor_handle),
        ],
        extend(registry! {
            scope(App) [
                provide(instance(pool)),
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

/// Wrap the concrete manager as the backend-agnostic handle. Shares the same
/// instance the finalizer rolls back.
async fn provide_transaction_manager_handle(
    Inject(manager): Inject<PgTransactionManager>,
) -> InstantiatorResult<TransactionManagerHandle> {
    let manager: Arc<dyn TransactionManager> = manager;
    Ok(TransactionManagerHandle(manager))
}

/// Write side: runs on the shared request transaction (never commits).
async fn provide_command_gateway_handle(
    Inject(slot): Inject<TxSlot>,
    Inject(factory): Inject<TransparencyLogFactory>,
) -> InstantiatorResult<LogCommandGatewayHandle> {
    let gateway: Arc<dyn LogCommandGateway> =
        Arc::new(PostgresLogCommandGateway::new(slot, (*factory).clone()));
    Ok(LogCommandGatewayHandle(gateway))
}

/// Read side: queries the pool directly and rebuilds proofs with the hasher.
async fn provide_query_gateway_handle(
    Inject(pool): Inject<PgPool>,
    Inject(hasher): Inject<MerkleHasher>,
) -> InstantiatorResult<LogQueryGatewayHandle> {
    let gateway: Arc<dyn LogQueryGateway> = Arc::new(PostgresLogQueryGateway::new(
        (*pool).clone(),
        (*hasher).clone(),
    ));
    Ok(LogQueryGatewayHandle(gateway))
}

/// Durable checkpoint anchor: persists signed tree heads on the shared request
/// transaction (atomic with the checkpoint's issue).
async fn provide_checkpoint_anchor_handle(
    Inject(slot): Inject<TxSlot>,
) -> InstantiatorResult<CheckpointAnchorHandle> {
    let anchor: Arc<dyn CheckpointAnchor> = Arc::new(PostgresCheckpointAnchor::new(slot));
    Ok(CheckpointAnchorHandle(anchor))
}

/// Safety net: roll back a transaction left open when a request scope closes.
async fn rollback_open_transaction(manager: Arc<PgTransactionManager>) {
    let _ = manager.rollback().await;
}
