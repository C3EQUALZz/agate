//! Scenario tests over the public API: append → prove → verify, and tamper detection.

use agate_audit::domain::common::values::Timestamp;
use agate_audit::domain::merkle::{
    CheckpointSigner, CheckpointVerifier, LeafIndex, LogId, MerkleHasher, MerkleProofs,
    TransparencyLog, TransparencyLogFactory,
};
use agate_crypto::ed25519::{Ed25519Signer, Ed25519Verifier};
use agate_crypto::{CryptoRegistry, HashAlgo, KeyId};
use uuid::Uuid;

fn new_log() -> TransparencyLog {
    TransparencyLogFactory::new(CryptoRegistry::hasher(HashAlgo::Sha256).unwrap())
        .create(LogId(Uuid::nil()), Timestamp::from_millis(0).unwrap())
}

fn merkle_hasher() -> MerkleHasher {
    MerkleHasher::new(CryptoRegistry::hasher(HashAlgo::Sha256).unwrap())
}

#[test]
fn every_record_has_a_verifiable_inclusion_proof() {
    let mut log = new_log();
    let records: [&[u8]; 5] = [b"a", b"b", b"c", b"d", b"e"];
    for r in records {
        log.append(r);
    }
    let root = log.root();
    let mh = merkle_hasher();

    for i in 0..records.len() {
        let proof = log.prove_inclusion(LeafIndex(i as u64)).unwrap();
        let leaf = &log.leaf_hashes()[i];
        assert!(MerkleProofs::verify_inclusion(&mh, &proof, leaf, &root));
    }
}

#[test]
fn tampered_leaf_fails_inclusion() {
    let mut log = new_log();
    log.append(b"a");
    log.append(b"b");
    let root = log.root();
    let mh = merkle_hasher();

    let proof = log.prove_inclusion(LeafIndex(0)).unwrap();
    let forged = mh.leaf(b"forged");
    assert!(!MerkleProofs::verify_inclusion(&mh, &proof, &forged, &root));
}

#[test]
fn append_only_growth_is_consistent() {
    let mut log = new_log();
    let first: [&[u8]; 3] = [b"a", b"b", b"c"];
    for r in first {
        log.append(r);
    }
    let old_root = log.root();
    let old_size = log.size();

    log.append(b"d");
    log.append(b"e");
    let new_root = log.root();

    let proof = log.prove_consistency(old_size).unwrap();
    let mh = merkle_hasher();
    assert!(MerkleProofs::verify_consistency(&mh, &proof, &old_root, &new_root));
}

#[test]
fn signed_checkpoint_verifies_and_rejects_wrong_key() {
    let mut log = new_log();
    log.append(b"a");
    log.append(b"b");
    let head = log.issue_checkpoint(Timestamp::from_millis(1_000).unwrap());

    let signer = Ed25519Signer::from_seed(&[42u8; 32], KeyId("test-key".to_string()));
    let sth = CheckpointSigner::sign(&signer, &head);

    let verifier = Ed25519Verifier::from_public_bytes(&signer.verifying_key_bytes()).unwrap();
    assert!(CheckpointVerifier::verify(&verifier, &sth));

    let other = Ed25519Signer::from_seed(&[7u8; 32], KeyId("other".to_string()));
    let other_verifier = Ed25519Verifier::from_public_bytes(&other.verifying_key_bytes()).unwrap();
    assert!(!CheckpointVerifier::verify(&other_verifier, &sth));
}
