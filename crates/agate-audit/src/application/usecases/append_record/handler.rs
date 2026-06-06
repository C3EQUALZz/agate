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
        let mut log = self
            .gateway
            .load(request.log)
            .await?
            .ok_or(AuditError::LogNotFound(request.log))?;
        let index = log.append(&request.record);
        self.gateway.save(&log).await?;
        debug!(log = %request.log.0, index = index.0, "appended record to transparency log");
        Ok(index)
    }
}
