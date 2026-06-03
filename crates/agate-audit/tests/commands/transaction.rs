use std::sync::Arc;
use std::sync::atomic::Ordering;

use agate_audit::application::common::behaviors::TransactionBehavior;
use agate_audit::application::common::messaging::{Behavior, Mediator, RequestHandler};
use agate_audit::application::usecases::append_record::{AppendRecord, AppendRecordHandler};
use agate_audit::domain::merkle::{LeafIndex, LogId};
use uuid::Uuid;

use crate::common::fakes::{InMemoryLogStore, RecordingTransactionManager};

#[tokio::test]
async fn command_commits_when_transaction_behavior_is_in_the_pipeline() {
    let store = Arc::new(InMemoryLogStore::new());
    let id = LogId(Uuid::nil());
    store.seed_empty(id);

    let tx = Arc::new(RecordingTransactionManager::new());
    let handler: Arc<dyn RequestHandler<AppendRecord>> = Arc::new(AppendRecordHandler::new(store));
    let behaviors: Vec<Arc<dyn Behavior<AppendRecord>>> =
        vec![Arc::new(TransactionBehavior::new(tx.clone()))];
    let mediator = Mediator::new(handler, behaviors);

    let index = mediator
        .send(AppendRecord {
            log: id,
            record: b"a".to_vec(),
        })
        .await
        .unwrap();

    assert_eq!(index, LeafIndex(0));
    assert_eq!(tx.begins.load(Ordering::SeqCst), 1);
    assert_eq!(tx.commits.load(Ordering::SeqCst), 1);
    assert_eq!(tx.rollbacks.load(Ordering::SeqCst), 0);
}
