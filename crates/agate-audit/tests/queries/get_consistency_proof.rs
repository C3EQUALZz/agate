use std::sync::Arc;

use agate_audit::application::common::messaging::{Mediator, RequestHandler};
use agate_audit::application::usecases::get_consistency_proof::{
    GetConsistencyProof, GetConsistencyProofHandler,
};
use agate_audit::domain::merkle::{LogId, MerkleProofs, TreeSize};
use uuid::Uuid;

use crate::common::factories::merkle_hasher;
use crate::common::fakes::InMemoryLogStore;

#[tokio::test]
async fn consistency_proof_view_verifies() {
    let store = Arc::new(InMemoryLogStore::new());
    let id = LogId(Uuid::nil());
    let records: [&[u8]; 3] = [b"a", b"b", b"c"];
    store.seed_with(id, &records);

    let handler: Arc<dyn RequestHandler<GetConsistencyProof>> =
        Arc::new(GetConsistencyProofHandler::new(store));

    let view = Mediator::without_behaviors(handler)
        .send(GetConsistencyProof {
            log: id,
            first: TreeSize(1),
        })
        .await
        .unwrap();

    let hasher = merkle_hasher();
    assert!(MerkleProofs::verify_consistency(
        &hasher,
        &view.proof,
        &view.old_root,
        &view.new_root,
    ));
}
