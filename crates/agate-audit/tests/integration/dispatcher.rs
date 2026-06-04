//! The froodi composition root driven through the [`Dispatcher`] against a real
//! database: one request scope (one transaction) per dispatch — the component
//! wiring, without HTTP (that is the e2e suite).

use std::sync::Arc;

use froodi::async_impl::Container;
use uuid::Uuid;

use agate_audit::application::common::messaging::{Dispatcher, Registry, Request};
use agate_audit::application::errors::AuditError;
use agate_audit::application::usecases::append_record::AppendRecord;
use agate_audit::application::usecases::create_log::CreateLog;
use agate_audit::application::usecases::get_consistency_proof::GetConsistencyProof;
use agate_audit::application::usecases::get_inclusion_proof::GetInclusionProof;
use agate_audit::domain::merkle::{LeafIndex, LogId, MerkleHasher, MerkleProofs, TreeSize};
use agate_audit::setup::ioc::{build_container, build_registry};
use agate_crypto::{CryptoRegistry, HashAlgo};

use crate::fixture::{Db, start};

/// The wired application: an App-scope container plus the routing table, over a
/// live database (the container is held by `_db` for RAII).
struct App {
    _db: Db,
    container: Container,
    registry: Arc<Registry<Container>>,
}

impl App {
    async fn new() -> Self {
        let db = start().await;
        let container = build_container(db.pool.clone());
        Self {
            _db: db,
            container,
            registry: Arc::new(build_registry()),
        }
    }

    /// Dispatch one request in its own request scope (one transaction), closing
    /// the scope afterwards so finalizers run.
    async fn dispatch<R: Request>(&self, request: R) -> R::Response {
        let scope = Arc::new(
            self.container
                .clone()
                .enter_build()
                .expect("open request scope"),
        );
        let dispatcher = Dispatcher::new(scope.clone(), self.registry.clone());
        let response = dispatcher.send(request).await;
        scope.close().await;
        response
    }
}

fn hasher() -> MerkleHasher {
    MerkleHasher::new(CryptoRegistry::hasher(HashAlgo::Sha256).unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_append_and_prove_through_the_container() {
    let app = App::new().await;

    let log = app.dispatch(CreateLog).await.unwrap();

    let first = app
        .dispatch(AppendRecord {
            log,
            record: b"a".to_vec(),
        })
        .await
        .unwrap();
    assert_eq!(first, LeafIndex(0));

    for record in [b"b".to_vec(), b"c".to_vec()] {
        app.dispatch(AppendRecord { log, record }).await.unwrap();
    }

    let inclusion = app
        .dispatch(GetInclusionProof {
            log,
            index: LeafIndex(1),
        })
        .await
        .unwrap();
    assert!(MerkleProofs::verify_inclusion(
        &hasher(),
        &inclusion.proof,
        &inclusion.leaf_hash,
        &inclusion.root,
    ));

    let consistency = app
        .dispatch(GetConsistencyProof {
            log,
            first: TreeSize(1),
        })
        .await
        .unwrap();
    assert!(MerkleProofs::verify_consistency(
        &hasher(),
        &consistency.proof,
        &consistency.old_root,
        &consistency.new_root,
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_command_rolls_back_and_reports_not_found() {
    let app = App::new().await;
    let missing = LogId(Uuid::new_v4());

    let result = app
        .dispatch(AppendRecord {
            log: missing,
            record: b"x".to_vec(),
        })
        .await;

    assert!(matches!(result, Err(AuditError::LogNotFound(id)) if id == missing));
}
