use async_trait::async_trait;

use crate::application::common::query_models::{ConsistencyProofView, InclusionProofView};
use crate::application::errors::AuditError;
use crate::domain::merkle::{LeafIndex, LogId, TreeSize};

/// Read-side gateway: returns frontend-friendly read models (DTOs), not the
/// aggregate. May be served from a projection/cache rather than the write store.
#[async_trait]
pub trait LogQueryGateway: Send + Sync {
    async fn inclusion_proof(
        &self,
        id: LogId,
        index: LeafIndex,
    ) -> Result<InclusionProofView, AuditError>;

    async fn consistency_proof(
        &self,
        id: LogId,
        first: TreeSize,
    ) -> Result<ConsistencyProofView, AuditError>;
}
