//! Application providers: the use-case handlers and the pipeline behavior.
//!
//! Handlers depend on *port* trait objects, so each instantiator resolves the
//! concrete adapter and coerces it (`Arc<Concrete>` → `Arc<dyn Port>`) at the
//! constructor call. All are Request-scoped: they share the request's
//! transaction and gateways.

use std::sync::Arc;

use froodi::{
    DefaultScope::Request, Inject, InstantiatorResult, async_impl::RegistryWithSync, async_registry,
};

use crate::application::common::behaviors::{
    MetricsBehavior, TracingBehavior, TransactionBehavior,
};
use crate::application::common::ports::{AuditMetrics, KeyStore};
use crate::application::usecases::append_record::AppendRecordHandler;
use crate::application::usecases::create_log::CreateLogHandler;
use crate::application::usecases::get_consistency_proof::GetConsistencyProofHandler;
use crate::application::usecases::get_inclusion_proof::GetInclusionProofHandler;
use crate::application::usecases::issue_checkpoint::IssueCheckpointHandler;
use crate::domain::merkle::{LogId, TransparencyLogFactory};
use crate::domain::ports::{Clock, IdGenerator};
use crate::infrastructure::{
    AuditMetricsRecorder, Ed25519KeyStore, SystemClock, UuidLogIdGenerator,
};
use crate::setup::ioc::handles::{
    CheckpointAnchorHandle, LogCommandGatewayHandle, LogQueryGatewayHandle,
    TransactionManagerHandle,
};

/// The use-case handlers and the transaction behavior, all Request-scoped.
pub(crate) fn handler_providers() -> RegistryWithSync {
    async_registry! {
        scope(Request) [
            provide(provide_tracing_behavior),
            provide(provide_transaction_behavior),
            provide(provide_metrics_behavior),
            provide(provide_create_log_handler),
            provide(provide_append_record_handler),
            provide(provide_issue_checkpoint_handler),
            provide(provide_get_inclusion_proof_handler),
            provide(provide_get_consistency_proof_handler),
        ],
    }
}

async fn provide_issue_checkpoint_handler(
    Inject(gateway): Inject<LogCommandGatewayHandle>,
    Inject(keys): Inject<Ed25519KeyStore>,
    Inject(anchor): Inject<CheckpointAnchorHandle>,
    Inject(clock): Inject<SystemClock>,
) -> InstantiatorResult<IssueCheckpointHandler> {
    let keys: Arc<dyn KeyStore> = keys;
    let clock: Arc<dyn Clock> = clock;
    Ok(IssueCheckpointHandler::new(
        gateway.0.clone(),
        keys,
        anchor.0.clone(),
        clock,
    ))
}

async fn provide_transaction_behavior(
    Inject(manager): Inject<TransactionManagerHandle>,
) -> InstantiatorResult<TransactionBehavior> {
    Ok(TransactionBehavior::new(manager.0.clone()))
}

async fn provide_tracing_behavior() -> InstantiatorResult<TracingBehavior> {
    Ok(TracingBehavior)
}

async fn provide_metrics_behavior() -> InstantiatorResult<MetricsBehavior> {
    let metrics: Arc<dyn AuditMetrics> = Arc::new(AuditMetricsRecorder);
    Ok(MetricsBehavior::new(metrics))
}

async fn provide_create_log_handler(
    Inject(factory): Inject<TransparencyLogFactory>,
    Inject(ids): Inject<UuidLogIdGenerator>,
    Inject(clock): Inject<SystemClock>,
    Inject(gateway): Inject<LogCommandGatewayHandle>,
) -> InstantiatorResult<CreateLogHandler> {
    let ids: Arc<dyn IdGenerator<LogId>> = ids;
    let clock: Arc<dyn Clock> = clock;
    Ok(CreateLogHandler::new(
        (*factory).clone(),
        ids,
        clock,
        gateway.0.clone(),
    ))
}

async fn provide_append_record_handler(
    Inject(gateway): Inject<LogCommandGatewayHandle>,
) -> InstantiatorResult<AppendRecordHandler> {
    Ok(AppendRecordHandler::new(gateway.0.clone()))
}

async fn provide_get_inclusion_proof_handler(
    Inject(gateway): Inject<LogQueryGatewayHandle>,
) -> InstantiatorResult<GetInclusionProofHandler> {
    Ok(GetInclusionProofHandler::new(gateway.0.clone()))
}

async fn provide_get_consistency_proof_handler(
    Inject(gateway): Inject<LogQueryGatewayHandle>,
) -> InstantiatorResult<GetConsistencyProofHandler> {
    Ok(GetConsistencyProofHandler::new(gateway.0.clone()))
}
