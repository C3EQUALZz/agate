use crate::application::common::messaging::{Query, Request};
use crate::application::common::query_models::ConsistencyProofView;
use crate::application::errors::AuditError;
use crate::domain::merkle::{LogId, TreeSize};

/// Produce an RFC 6962 consistency proof between `first` and the current size.
pub struct GetConsistencyProof {
    pub log: LogId,
    pub first: TreeSize,
}

impl Request for GetConsistencyProof {
    type Response = Result<ConsistencyProofView, AuditError>;
}

impl Query for GetConsistencyProof {}
