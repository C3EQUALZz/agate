//! End-to-end fixture: a real PostgreSQL (testcontainers), a stub AG-UI agent,
//! and the full server (proxy + audit outbox) booted on an ephemeral port.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::header::CONTENT_TYPE;
use axum::routing::post;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use testcontainers::ContainerAsync;
use testcontainers::ImageExt;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use tokio::net::TcpListener;

/// Pinned Postgres image — a current LTS, not testcontainers' EOL default.
const POSTGRES_IMAGE_TAG: &str = "17-alpine";

use agate_audit::application::common::messaging::{Dispatcher, Registry, Request};
use agate_audit::application::usecases::create_log::CreateLog;
use agate_audit::application::usecases::get_inclusion_proof::GetInclusionProof;
use agate_audit::domain::merkle::{LeafIndex, LogId};
use agate_audit::infrastructure::persistence::postgres::run_migrations;
use agate_audit::setup::ioc::{build_container, build_registry};
use agate_audit::setup::storage::Storage;
use agate_policy::application::PolicyService;
use agate_policy::domain::decision::PolicyRuleset;
use agate_proxy::application::common::ports::PolicyPort;
use agate_proxy::infrastructure::FailMode;
use agate_proxy::setup::configs::ProxyConfig;
use agate_server::infrastructure::audit::FullPolicy;
use agate_server::infrastructure::policy::PolicyAdapter;
use agate_server::setup::bootstrap::{
    OutboxSettings, Server, ServerConfig, Supervisor, build_server,
};
use froodi::async_impl::Container;

/// A run the proxy inspects into three recordable events: a lifecycle start, a
/// message chunk, and a lifecycle finish — leaves 0, 1, 2 of the log.
pub const SSE_BODY: &str = concat!(
    "data: {\"type\":\"RUN_STARTED\"}\n\n",
    "data: {\"type\":\"TEXT_MESSAGE_CONTENT\",\"messageId\":\"m1\",\"delta\":\"hello\"}\n\n",
    "data: {\"type\":\"RUN_FINISHED\"}\n\n",
);

/// A booted server with the pieces a test needs: the proxy's base URL, a pool to
/// inspect the database directly, and the log it records to. Holds the container
/// alive (RAII).
pub struct TestServer {
    pub container: ContainerAsync<Postgres>,
    pub pool: PgPool,
    pub base_url: String,
    pub log: LogId,
}

/// Boot the stub agent (answering with `sse_body`), the database, and the
/// server wired to `ruleset` (the static policy adapter); create a fresh log.
pub async fn spawn(ruleset: PolicyRuleset, sse_body: &'static str) -> TestServer {
    let policy = Arc::new(PolicyAdapter::new(PolicyService::new(ruleset)));
    spawn_with_policy(policy, sse_body).await
}

/// Boot the stub agent, the database, and the server wired to an already-built
/// decision engine — so the CEL and Rego plugin backends are exercised through
/// the live proxy → audit path, not just the static ruleset.
pub async fn spawn_with_policy(policy: Arc<dyn PolicyPort>, sse_body: &'static str) -> TestServer {
    let container = Postgres::default()
        .with_tag(POSTGRES_IMAGE_TAG)
        .start()
        .await
        .unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = PgPoolOptions::new().connect(&url).await.unwrap();
    run_migrations(&pool).await.unwrap();

    let registry = Arc::new(build_registry());
    let log = dispatch(&audit_container(pool.clone()), &registry, CreateLog)
        .await
        .unwrap();

    let agent_endpoint = stub_agent(sse_body).await;
    let proxy = ProxyConfig::new(agent_endpoint, "127.0.0.1:0".to_string());
    // The outbox task is detached: it lives as long as the served app (and the
    // audit sink inside it) keeps the channel open.
    let storage = Storage::postgres(pool.clone());
    // The test does not drive graceful shutdown, so the supervisor exists only to
    // satisfy `build_server` and is intentionally not awaited: the supervised
    // outbox lives as long as the served app keeps the audit-sink channel open,
    // and assertions gate on `poll_inclusion`, which only sees durably committed
    // records.
    let supervisor = Supervisor::new();
    let Server { app } = build_server(
        &storage,
        ServerConfig {
            proxy,
            log,
            policy,
            fail_mode: FailMode::Closed,
            decision_timeout: Duration::from_secs(5),
            checkpoint: None,
            outbox: OutboxSettings {
                capacity: 1024,
                on_full: FullPolicy::Block,
            },
        },
        &supervisor,
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    TestServer {
        container,
        pool,
        base_url: format!("http://{addr}"),
        log,
    }
}

/// Boot a stub AG-UI agent answering `POST /run` with `body`.
async fn stub_agent(body: &'static str) -> String {
    let app = Router::new().route(
        "/run",
        post(move || async move { ([(CONTENT_TYPE, "text/event-stream")], body) }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}/run")
}

/// A fresh audit IoC container over `pool` — for driving queries against the log
/// directly in assertions (separate from the server's own container).
#[must_use]
pub fn audit_container(pool: PgPool) -> Container {
    build_container(&Storage::postgres(pool))
}

#[must_use]
pub fn audit_registry() -> Arc<Registry<Container>> {
    Arc::new(build_registry())
}

/// Dispatch one audit request in its own request scope (one transaction).
pub async fn dispatch<R: Request>(
    container: &Container,
    registry: &Arc<Registry<Container>>,
    request: R,
) -> R::Response {
    let scope = Arc::new(container.clone().enter_build().expect("open request scope"));
    let dispatcher = Dispatcher::new(scope.clone(), registry.clone());
    let response = dispatcher.send(request).await;
    scope.close().await;
    response
}

/// A reqwest client with a request timeout, so a stalled proxy or agent fails
/// the test instead of hanging CI.
#[must_use]
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("build reqwest client")
}

/// Outbox writes are asynchronous; poll the log up to ~5s (50 × 100ms).
const POLL_ATTEMPTS: usize = 50;
const POLL_INTERVAL_MS: u64 = 100;

/// Poll the log for an inclusion proof of `index`, tolerating the outbox's
/// asynchronous write. Returns whether the leaf appeared within the timeout.
pub async fn poll_inclusion(
    container: &Container,
    registry: &Arc<Registry<Container>>,
    log: LogId,
    index: LeafIndex,
) -> bool {
    poll_inclusion_within(container, registry, log, index, POLL_ATTEMPTS).await
}

/// Like [`poll_inclusion`] but with an explicit attempt budget (× 100ms), for
/// tests that queue many records and so need a longer drain window than the
/// default — a slow CI runner appends the outbox serially.
pub async fn poll_inclusion_within(
    container: &Container,
    registry: &Arc<Registry<Container>>,
    log: LogId,
    index: LeafIndex,
    attempts: usize,
) -> bool {
    for _ in 0..attempts {
        if dispatch(container, registry, GetInclusionProof { log, index })
            .await
            .is_ok()
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
    false
}
