use std::sync::Arc;

use agate_audit::application::common::messaging::{Mediator, RequestHandler};
use agate_audit::application::errors::AuditError;
use agate_audit::application::usecases::append_record::{AppendRecord, AppendRecordHandler};
use agate_audit::domain::merkle::{LeafIndex, LogId};
use uuid::Uuid;

use crate::common::fakes::InMemoryLogStore;

#[tokio::test]
async fn append_assigns_monotonic_indices() {
    let store = Arc::new(InMemoryLogStore::new());
    let id = LogId(Uuid::nil());
    store.seed_empty(id);

    let handler: Arc<dyn RequestHandler<AppendRecord>> = Arc::new(AppendRecordHandler::new(store));
    let mediator = Mediator::without_behaviors(handler);

    let first = mediator
        .send(AppendRecord {
            log: id,
            record: b"a".to_vec(),
        })
        .await
        .unwrap();
    let second = mediator
        .send(AppendRecord {
            log: id,
            record: b"b".to_vec(),
        })
        .await
        .unwrap();

    assert_eq!(first, LeafIndex(0));
    assert_eq!(second, LeafIndex(1));
}

#[tokio::test]
async fn append_to_missing_log_fails() {
    let store = Arc::new(InMemoryLogStore::new());
    let handler: Arc<dyn RequestHandler<AppendRecord>> = Arc::new(AppendRecordHandler::new(store));

    let result = Mediator::without_behaviors(handler)
        .send(AppendRecord {
            log: LogId(Uuid::nil()),
            record: b"x".to_vec(),
        })
        .await;

    assert!(matches!(result, Err(AuditError::LogNotFound(_))));
}
