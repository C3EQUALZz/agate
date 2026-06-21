use std::sync::Arc;

use async_trait::async_trait;
use tracing::debug;

use super::command::AppendRecord;
use crate::application::common::messaging::RequestHandler;
use crate::application::common::ports::LogCommandGateway;
use crate::application::errors::AuditError;
use crate::domain::merkle::LeafIndex;

pub struct AppendRecordHandler {
    gateway: Arc<dyn LogCommandGateway>,
}

impl AppendRecordHandler {
    pub fn new(gateway: Arc<dyn LogCommandGateway>) -> Self {
        Self { gateway }
    }
}

#[async_trait]
impl RequestHandler<AppendRecord> for AppendRecordHandler {
    async fn handle(&self, request: AppendRecord) -> Result<LeafIndex, AuditError> {
        // Append one leaf in place (O(1)) rather than load-all → append → save-all
        // (O(n) per call → O(n²) over the log's life). The root and proofs are
        // computed on demand from the stored leaves by the checkpoint/query paths.
        let index = self
            .gateway
            .append_record(request.log, &request.record)
            .await?
            .ok_or(AuditError::LogNotFound(request.log))?;
        debug!(log = %request.log.0, index = index.0, "appended record to transparency log");
        Ok(index)
    }
}
