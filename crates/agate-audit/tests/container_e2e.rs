//! End-to-end test of the `froodi` composition root against a real database
//! (testcontainers; requires Docker). Drives use cases through the
//! [`Dispatcher`] exactly as a presentation adapter would: one request scope
//! per dispatch, each with its own transaction.

use std::sync::Arc;

use froodi::async_impl::Container;
use rstest::{fixture, rstest};
use sqlx::postgres::PgPoolOptions;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

use agate_audit::application::common::messaging::{Dispatcher, Registry, Request};
use agate_audit::application::errors::AuditError;
use agate_audit::application::usecases::append_record::AppendRecord;
use agate_audit::application::usecases::create_log::CreateLog;
use agate_audit::application::usecases::get_consistency_proof::GetConsistencyProof;
use agate_audit::application::usecases::get_inclusion_proof::GetInclusionProof;
use agate_audit::domain::merkle::{LeafIndex, LogId, MerkleHasher, MerkleProofs, TreeSize};
use agate_audit::infrastructure::di::{build_container, build_registry};
use agate_audit::infrastructure::persistence::postgres::run_migrations;
use agate_crypto::{CryptoRegistry, HashAlgo};

/// The wired application: an App-scope container plus the routing table. Holds
/// the Postgres container alive (RAII).
struct App {
    _container: ContainerAsync<Postgres>,
    container: Container,
    registry: Arc<Registry<Container>>,
}

impl App {
    /// Dispatch one request in its own request scope (one transaction), closing
    /// the scope afterwards so finalizers run — exactly one request's lifecycle.
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

#[fixture]
async fn app() -> App {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = PgPoolOptions::new().connect(&url).await.unwrap();
    run_migrations(&pool).await.unwrap();
    App {
        _container: container,
        container: build_container(pool),
        registry: Arc::new(build_registry()),
    }
}

fn hasher() -> MerkleHasher {
    MerkleHasher::new(CryptoRegistry::hasher(HashAlgo::Sha256).unwrap())
}

#[rstest]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_append_and_prove_through_the_container(#[future] app: App) {
    let app = app.await;

    // Each command commits in its own request scope; the next scope loads the
    // committed state — proving the transaction boundary is wired correctly.
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

    // Read side (separate connection, no transaction) sees all three records.
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

#[rstest]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_command_rolls_back_and_reports_not_found(#[future] app: App) {
    let app = app.await;
    let missing = LogId(Uuid::new_v4());

    let result = app
        .dispatch(AppendRecord {
            log: missing,
            record: b"x".to_vec(),
        })
        .await;

    // The handler errors (log absent); TransactionBehavior rolls back and
    // surfaces the original error through the container.
    assert!(matches!(result, Err(AuditError::LogNotFound(id)) if id == missing));
}
