//! The durable checkpoint anchor against a real database: a signed tree head is
//! persisted to `audit_checkpoint` on the request transaction and survives the
//! commit, idempotently per (log, size).

use std::sync::Arc;

use uuid::Uuid;

use agate_audit::application::common::ports::{CheckpointAnchor, TransactionManager};
use agate_audit::domain::common::values::Timestamp;
use agate_audit::domain::merkle::{LogId, SignedTreeHead, TreeHead, TreeSize};
use agate_audit::infrastructure::persistence::log::postgres::PostgresCheckpointAnchor;
use agate_audit::infrastructure::persistence::postgres::{
    PgTransactionManager, SharedTransaction, TxSlot,
};
use agate_crypto::{Digest, HashAlgo, KeyId, SignAlgo, Signature};

use crate::fixture::start;

fn sample_sth() -> SignedTreeHead {
    SignedTreeHead {
        head: TreeHead {
            size: TreeSize(3),
            root: Digest {
                algo: HashAlgo::Sha256,
                bytes: vec![1, 2, 3],
            },
            at: Timestamp::from_millis(42).expect("valid timestamp"),
        },
        signature: Signature {
            algo: SignAlgo::Ed25519,
            key_id: KeyId("k1".to_owned()),
            bytes: vec![9; 64],
        },
    }
}

/// Anchor a checkpoint on a request transaction and commit it.
async fn anchor_committed(pool: &sqlx::PgPool, log: LogId, sth: &SignedTreeHead) {
    let slot: SharedTransaction = Arc::new(TxSlot::new(None));
    let manager = PgTransactionManager::new(pool.clone(), Arc::clone(&slot));
    manager.begin().await.expect("begin");
    PostgresCheckpointAnchor::new(Arc::clone(&slot))
        .anchor(log, sth)
        .await
        .expect("anchor");
    manager.commit().await.expect("commit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anchoring_persists_the_checkpoint_idempotently() {
    let db = start().await;
    let pool = db.pool.clone();

    // A log row for the foreign key.
    let log = LogId(Uuid::new_v4());
    sqlx::query(
        "INSERT INTO audit_log (id, created_at, updated_at, hash_algo) VALUES ($1, 0, 0, $2)",
    )
    .bind(log.0)
    .bind(i16::from(HashAlgo::Sha256.code()))
    .execute(&pool)
    .await
    .expect("insert log");

    let sth = sample_sth();
    anchor_committed(&pool, log, &sth).await;

    // It persisted with the committed size + signing key.
    let (size, key_id): (i64, String) =
        sqlx::query_as("SELECT tree_size, key_id FROM audit_checkpoint WHERE log_id = $1")
            .bind(log.0)
            .fetch_one(&pool)
            .await
            .expect("fetch checkpoint");
    assert_eq!(size, 3);
    assert_eq!(key_id, "k1");

    // Re-anchoring the same size is a no-op (ON CONFLICT DO NOTHING), not a dup.
    anchor_committed(&pool, log, &sth).await;
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM audit_checkpoint WHERE log_id = $1")
            .bind(log.0)
            .fetch_one(&pool)
            .await
            .expect("count checkpoints");
    assert_eq!(count, 1, "re-anchoring the same size must not duplicate");
}
