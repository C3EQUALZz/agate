use std::sync::Arc;

use async_trait::async_trait;
use tracing::info;

use super::command::CreateLog;
use crate::application::common::messaging::RequestHandler;
use crate::application::common::ports::LogCommandGateway;
use crate::application::errors::AuditError;
use crate::domain::merkle::{LogId, TransparencyLogFactory};
use crate::domain::ports::{Clock, IdGenerator};

pub struct CreateLogHandler {
    factory: TransparencyLogFactory,
    ids: Arc<dyn IdGenerator<LogId>>,
    clock: Arc<dyn Clock>,
    gateway: Arc<dyn LogCommandGateway>,
}

impl CreateLogHandler {
    pub fn new(
        factory: TransparencyLogFactory,
        ids: Arc<dyn IdGenerator<LogId>>,
        clock: Arc<dyn Clock>,
        gateway: Arc<dyn LogCommandGateway>,
    ) -> Self {
        Self {
            factory,
            ids,
            clock,
            gateway,
        }
    }
}

#[async_trait]
impl RequestHandler<CreateLog> for CreateLogHandler {
    async fn handle(&self, _request: CreateLog) -> Result<LogId, AuditError> {
        let id = self.ids.generate();
        let log = self.factory.create(id, self.clock.now());
        self.gateway.save(&log).await?;
        info!(log = %id.0, "created transparency log");
        Ok(id)
    }
}
