use std::sync::Arc;

use agate_audit::application::common::messaging::{Mediator, RequestHandler};
use agate_audit::application::common::ports::KeyStore;
use agate_audit::application::usecases::issue_checkpoint::{
    IssueCheckpoint, IssueCheckpointHandler,
};
use agate_audit::domain::merkle::{CheckpointVerifier, LogId, SignedTreeHead, TreeSize};
use agate_crypto::KeyId;
use uuid::Uuid;

use crate::common::factories::epoch;
use crate::common::fakes::{FakeKeyStore, FixedClock, InMemoryLogStore, RecordingAnchor};

/// A three-record log wired to a checkpoint handler, plus the key store and
/// anchor so a test can assert what was signed and anchored.
struct Fixture {
    handler: Arc<dyn RequestHandler<IssueCheckpoint>>,
    keys: Arc<FakeKeyStore>,
    anchor: Arc<RecordingAnchor>,
    id: LogId,
    key: KeyId,
}

fn fixture() -> Fixture {
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
    Fixture {
        handler,
        keys,
        anchor,
        id,
        key: KeyId("test-key".to_string()),
    }
}

async fn issue(fx: &Fixture, previous_size: Option<TreeSize>) -> SignedTreeHead {
    Mediator::without_behaviors(fx.handler.clone())
        .send(IssueCheckpoint {
            log: fx.id,
            key: fx.key.clone(),
            previous_size,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn issue_checkpoint_signs_and_anchors() {
    let fx = fixture();
    let sth = issue(&fx, None).await;

    // The checkpoint commits to the current tree (3 records)...
    assert_eq!(sth.head.size, TreeSize(3));
    // ...was anchored exactly once...
    assert_eq!(fx.anchor.anchored.lock().unwrap().len(), 1);
    // ...and verifies against the key store's public key.
    let verifier = fx.keys.verifier(&fx.key).await.unwrap();
    assert!(CheckpointVerifier::verify(verifier.as_ref(), &sth));
}

#[tokio::test]
async fn issue_checkpoint_skips_anchoring_when_the_tree_is_unchanged() {
    let fx = fixture();

    // The issuer already holds a checkpoint at the current size (3): a signed
    // head still comes back, but nothing is re-anchored.
    let sth = issue(&fx, Some(TreeSize(3))).await;

    assert_eq!(sth.head.size, TreeSize(3));
    assert!(
        fx.anchor.anchored.lock().unwrap().is_empty(),
        "an unchanged tree is not re-anchored"
    );
    let verifier = fx.keys.verifier(&fx.key).await.unwrap();
    assert!(CheckpointVerifier::verify(verifier.as_ref(), &sth));
}
