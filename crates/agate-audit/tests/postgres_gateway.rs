//! PostgreSQL integration tests against a real database (testcontainers;
//! requires Docker). Uses `rstest` fixtures (pytest-style injection).

use std::sync::Arc;

use rstest::{fixture, rstest};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

use agate_audit::application::common::ports::{LogCommandGateway, LogQueryGateway};
use agate_audit::domain::common::values::Timestamp;
use agate_audit::domain::merkle::{
    LeafIndex, LogId, MerkleHasher, MerkleProofs, TransparencyLogFactory, TreeSize,
};
use agate_audit::infrastructure::persistence::log::postgres::{
    PostgresLogCommandGateway, PostgresLogQueryGateway, run_migrations,
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
    fn command(&self) -> PostgresLogCommandGateway {
        PostgresLogCommandGateway::new(self.pool.clone(), self.factory.clone())
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
async fn persists_log_and_serves_proofs(#[future] db: Db) {
    let db = db.await;
    let command = db.command();
    let query = db.query();

    let id = LogId(Uuid::new_v4());

    // Create an empty log, then append (append-only; two saves).
    let mut log = db.factory.create(id, Timestamp::from_millis(0).unwrap());
    command.save(&log).await.unwrap();
    let records: [&[u8]; 3] = [b"a", b"b", b"c"];
    for record in records {
        log.append(record);
    }
    command.save(&log).await.unwrap();

    // Reload from Postgres and compare to the in-memory aggregate.
    let loaded = command.load(id).await.unwrap().unwrap();
    assert_eq!(loaded.size(), TreeSize(3));
    assert_eq!(loaded.root(), log.root());

    // Inclusion proof verifies.
    let inclusion = query.inclusion_proof(id, LeafIndex(1)).await.unwrap();
    assert!(MerkleProofs::verify_inclusion(
        &db.hasher,
        &inclusion.proof,
        &inclusion.leaf_hash,
        &inclusion.root,
    ));

    // Consistency proof between size 1 and the current size (3) verifies.
    let consistency = query.consistency_proof(id, TreeSize(1)).await.unwrap();
    assert!(MerkleProofs::verify_consistency(
        &db.hasher,
        &consistency.proof,
        &consistency.old_root,
        &consistency.new_root,
    ));
}
