use std::sync::Arc;

use agate_audit::application::common::messaging::{Mediator, RequestHandler};
use agate_audit::application::common::ports::KeyStore;
use agate_audit::application::usecases::issue_checkpoint::{
    IssueCheckpoint, IssueCheckpointHandler,
};
use agate_audit::domain::merkle::{CheckpointVerifier, LogId, TreeSize};
use agate_crypto::KeyId;
use uuid::Uuid;

use crate::common::factories::epoch;
use crate::common::fakes::{FakeKeyStore, FixedClock, InMemoryLogStore, RecordingAnchor};

#[tokio::test]
async fn issue_checkpoint_signs_and_anchors() {
    let store = Arc::new(InMemoryLogStore::new());
    let id = LogId(Uuid::nil());
    let records: [&[u8]; 3] = [b"a", b"b", b"c"];
    store.seed_with(id, &records);

    let keys = Arc::new(FakeKeyStore::new([7u8; 32]));
    let anchor = Arc::new(RecordingAnchor::new());

    let handler: Arc<dyn RequestHandler<IssueCheckpoint>> = Arc::new(IssueCheckpointHandler::new(
        store,
        keys.clone(),
        anchor.clone(),
        Arc::new(FixedClock(epoch())),
    ));

    let key = KeyId("test-key".to_string());
    let sth = Mediator::without_behaviors(handler)
        .send(IssueCheckpoint {
            log: id,
            key: key.clone(),
            previous_size: None,
        })
        .await
        .unwrap();

    // The checkpoint commits to the current tree (3 records)...
    assert_eq!(sth.head.size, TreeSize(3));

    // ...was anchored exactly once...
    assert_eq!(anchor.anchored.lock().unwrap().len(), 1);

    // ...and verifies against the key store's public key.
    let verifier = keys.verifier(&key).await.unwrap();
    assert!(CheckpointVerifier::verify(verifier.as_ref(), &sth));
}

#[tokio::test]
async fn issue_checkpoint_skips_anchoring_when_the_tree_is_unchanged() {
    let store = Arc::new(InMemoryLogStore::new());
    let id = LogId(Uuid::nil());
    let records: [&[u8]; 3] = [b"a", b"b", b"c"];
    store.seed_with(id, &records);

    let keys = Arc::new(FakeKeyStore::new([7u8; 32]));
    let anchor = Arc::new(RecordingAnchor::new());

    let handler: Arc<dyn RequestHandler<IssueCheckpoint>> = Arc::new(IssueCheckpointHandler::new(
        store,
        keys.clone(),
        anchor.clone(),
        Arc::new(FixedClock(epoch())),
    ));
    let key = KeyId("test-key".to_string());

    // The issuer already holds a checkpoint at the current size (3): a signed
    // head still comes back, but nothing is re-anchored.
    let sth = Mediator::without_behaviors(handler)
        .send(IssueCheckpoint {
            log: id,
            key: key.clone(),
            previous_size: Some(TreeSize(3)),
        })
        .await
        .unwrap();

    assert_eq!(sth.head.size, TreeSize(3));
    assert!(
        anchor.anchored.lock().unwrap().is_empty(),
        "an unchanged tree is not re-anchored"
    );
    let verifier = keys.verifier(&key).await.unwrap();
    assert!(CheckpointVerifier::verify(verifier.as_ref(), &sth));
}
