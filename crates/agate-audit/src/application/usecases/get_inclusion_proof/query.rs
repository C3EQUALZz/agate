use crate::application::common::messaging::{Query, Request};
use crate::application::common::query_models::InclusionProofView;
use crate::application::errors::AuditError;
use crate::domain::merkle::{LeafIndex, LogId};

/// Produce an RFC 6962 inclusion proof for a leaf in a log.
pub struct GetInclusionProof {
    pub log: LogId,
    pub index: LeafIndex,
}

impl Request for GetInclusionProof {
    type Response = Result<InclusionProofView, AuditError>;
}

impl Query for GetInclusionProof {}
