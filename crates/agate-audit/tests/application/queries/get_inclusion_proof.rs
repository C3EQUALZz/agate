use std::sync::Arc;

use agate_audit::application::common::messaging::{Mediator, RequestHandler};
use agate_audit::application::usecases::get_inclusion_proof::{
    GetInclusionProof, GetInclusionProofHandler,
};
use agate_audit::domain::merkle::{LeafIndex, LogId, MerkleProofs};
use uuid::Uuid;

use crate::common::factories::merkle_hasher;
use crate::common::fakes::InMemoryLogStore;

#[tokio::test]
async fn inclusion_proof_view_verifies() {
    let store = Arc::new(InMemoryLogStore::new());
    let id = LogId(Uuid::nil());
    let records: [&[u8]; 3] = [b"a", b"b", b"c"];
    store.seed_with(id, &records);

    let handler: Arc<dyn RequestHandler<GetInclusionProof>> =
        Arc::new(GetInclusionProofHandler::new(store));

    let view = Mediator::without_behaviors(handler)
        .send(GetInclusionProof {
            log: id,
            index: LeafIndex(1),
        })
        .await
        .unwrap();

    let hasher = merkle_hasher();
    assert!(MerkleProofs::verify_inclusion(
        &hasher,
        &view.proof,
        &view.leaf_hash,
        &view.root,
    ));
}
