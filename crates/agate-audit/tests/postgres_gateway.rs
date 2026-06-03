//! PostgreSQL integration tests against a real database (testcontainers;
//! requires Docker). Uses `rstest` fixtures (pytest-style injection).

use std::sync::Arc;

use rstest::{fixture, rstest};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::Mutex;

use agate_audit::application::common::ports::{
    LogCommandGateway, LogQueryGateway, TransactionManager,
};
use agate_audit::application::errors::AuditError;
use agate_audit::domain::common::values::Timestamp;
use agate_audit::domain::merkle::{
    LeafIndex, LogId, MerkleHasher, MerkleProofs, TransparencyLogFactory, TreeSize,
};
use agate_audit::infrastructure::persistence::log::postgres::{
    PostgresLogCommandGateway, PostgresLogQueryGateway,
};
use agate_audit::infrastructure::persistence::postgres::{
    PgTransactionManager, SharedTransaction, run_migrations,
};
use agate_crypto::{CryptoRegistry, HashAlgo, Hasher};
use uuid::Uuid;

fn sha256() -> Arc<dyn Hasher> {
    CryptoRegistry::hasher(HashAlgo::Sha256).unwrap()
}

/// A running PostgreSQL container with migrations applied; holds the container
/// alive (RAII) and hands out gateways built against its pool.
struct Db {
    _container: ContainerAsync<Postgres>,
    pool: PgPool,
    factory: TransparencyLogFactory,
    hasher: MerkleHasher,
}

impl Db {
    /// A fresh shared transaction with a manager and a command gateway bound to
    /// it — mirrors one request scope, where both share one connection.
    fn transactional(&self) -> (PgTransactionManager, PostgresLogCommandGateway) {
        let transaction: SharedTransaction = Arc::new(Mutex::new(None));
        let manager = PgTransactionManager::new(self.pool.clone(), transaction.clone());
        let command = PostgresLogCommandGateway::new(transaction, self.factory.clone());
        (manager, command)
    }

    fn query(&self) -> PostgresLogQueryGateway {
        PostgresLogQueryGateway::new(self.pool.clone(), self.hasher.clone())
    }
}

#[fixture]
async fn db() -> Db {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = PgPoolOptions::new().connect(&url).await.unwrap();
    run_migrations(&pool).await.unwrap();
    Db {
        _container: container,
        pool,
        factory: TransparencyLogFactory::new(sha256()),
        hasher: MerkleHasher::new(sha256()),
    }
}

#[rstest]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn commit_persists_and_proofs_verify(#[future] db: Db) {
    let db = db.await;
    let id = LogId(Uuid::new_v4());

    // The manager opens and commits the transaction; the gateway only writes.
    let (transaction, command) = db.transactional();
    transaction.begin().await.unwrap();

    let mut log = db.factory.create(id, Timestamp::from_millis(0).unwrap());
    command.save(&log).await.unwrap();
    let records: [&[u8]; 3] = [b"a", b"b", b"c"];
    for record in records {
        log.append(record);
    }
    command.save(&log).await.unwrap();

    // Visible to the same transaction before the commit.
    let in_tx = command.load(id).await.unwrap().unwrap();
    assert_eq!(in_tx.size(), TreeSize(3));
    assert_eq!(in_tx.root(), log.root());

    transaction.commit().await.unwrap();

    // After the commit the read side (separate connection) sees the data.
    let inclusion = db.query().inclusion_proof(id, LeafIndex(1)).await.unwrap();
    assert!(MerkleProofs::verify_inclusion(
        &db.hasher,
        &inclusion.proof,
        &inclusion.leaf_hash,
        &inclusion.root,
    ));

    let consistency = db.query().consistency_proof(id, TreeSize(1)).await.unwrap();
    assert!(MerkleProofs::verify_consistency(
        &db.hasher,
        &consistency.proof,
        &consistency.old_root,
        &consistency.new_root,
    ));
}

#[rstest]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rollback_discards_writes(#[future] db: Db) {
    let db = db.await;
    let id = LogId(Uuid::new_v4());

    let (transaction, command) = db.transactional();
    transaction.begin().await.unwrap();
    let log = db.factory.create(id, Timestamp::from_millis(0).unwrap());
    command.save(&log).await.unwrap();
    transaction.rollback().await.unwrap();

    // Nothing was committed, so the read side never sees the log.
    let missing = db.query().inclusion_proof(id, LeafIndex(0)).await;
    assert!(matches!(missing, Err(AuditError::LogNotFound(_))));
}
