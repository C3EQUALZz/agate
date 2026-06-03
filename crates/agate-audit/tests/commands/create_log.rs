use std::sync::Arc;

use agate_audit::application::common::messaging::{Mediator, RequestHandler};
use agate_audit::application::usecases::create_log::{CreateLog, CreateLogHandler};
use agate_audit::domain::merkle::LogId;
use uuid::Uuid;

use crate::common::factories::{epoch, log_factory};
use crate::common::fakes::{FixedClock, FixedId, InMemoryLogStore};

#[tokio::test]
async fn create_log_returns_generated_id() {
    let store = Arc::new(InMemoryLogStore::new());
    let id = LogId(Uuid::nil());

    let handler: Arc<dyn RequestHandler<CreateLog>> = Arc::new(CreateLogHandler::new(
        log_factory(),
        Arc::new(FixedId(id)),
        Arc::new(FixedClock(epoch())),
        store.clone(),
    ));

    let created = Mediator::without_behaviors(handler)
        .send(CreateLog)
        .await
        .unwrap();

    assert_eq!(created, id);
}
