use std::sync::Arc;

use async_trait::async_trait;

use super::query::GetConsistencyProof;
use crate::application::common::messaging::RequestHandler;
use crate::application::common::ports::LogQueryGateway;
use crate::application::common::query_models::ConsistencyProofView;
use crate::application::errors::AuditError;

pub struct GetConsistencyProofHandler {
    gateway: Arc<dyn LogQueryGateway>,
}

impl GetConsistencyProofHandler {
    pub fn new(gateway: Arc<dyn LogQueryGateway>) -> Self {
        Self { gateway }
    }
}

#[async_trait]
impl RequestHandler<GetConsistencyProof> for GetConsistencyProofHandler {
    async fn handle(
        &self,
        request: GetConsistencyProof,
    ) -> Result<ConsistencyProofView, AuditError> {
        self.gateway
            .consistency_proof(request.log, request.first)
            .await
    }
}
