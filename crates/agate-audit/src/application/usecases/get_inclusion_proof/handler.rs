use std::sync::Arc;

use async_trait::async_trait;

use super::query::GetInclusionProof;
use crate::application::common::messaging::RequestHandler;
use crate::application::common::ports::LogQueryGateway;
use crate::application::common::query_models::InclusionProofView;
use crate::application::errors::AuditError;

pub struct GetInclusionProofHandler {
    gateway: Arc<dyn LogQueryGateway>,
}

impl GetInclusionProofHandler {
    pub fn new(gateway: Arc<dyn LogQueryGateway>) -> Self {
        Self { gateway }
    }
}

#[async_trait]
impl RequestHandler<GetInclusionProof> for GetInclusionProofHandler {
    async fn handle(&self, request: GetInclusionProof) -> Result<InclusionProofView, AuditError> {
        self.gateway
            .inclusion_proof(request.log, request.index)
            .await
    }
}
