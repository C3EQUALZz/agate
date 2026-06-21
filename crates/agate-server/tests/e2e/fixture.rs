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

/// A run that emits a secret in message text and then calls `rm_rf` — the agent
/// response the redact-and-deny e2e (static ruleset, CEL, and Rego) drive.
pub const REDACT_DENY_SSE: &str = concat!(
    "data: {\"type\":\"RUN_STARTED\"}\n\n",
    "data: {\"type\":\"TEXT_MESSAGE_CONTENT\",\"messageId\":\"m1\",\"delta\":\"token sk-LEAK end\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\",\"toolCallName\":\"rm_rf\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_ARGS\",\"toolCallId\":\"c1\",\"delta\":\"{}\"}\n\n",
    "data: {\"type\":\"TOOL_CALL_END\",\"toolCallId\":\"c1\"}\n\n",
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
    /// The background-task supervisor, so a test can drive graceful shutdown and
    /// await the outbox drain (see [`shutdown_and_drain`](TestServer::shutdown_and_drain)).
    supervisor: Supervisor,
    /// Signals the serve task to stop; dropping it (or sending) ends serving, so
    /// the app — and the audit sink inside it — drops and closes the outbox channel.
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl TestServer {
    /// Stop serving and await the supervised drain, exactly as `main` does on
    /// SIGTERM: end the serve task (dropping the app closes the outbox channel),
    /// then `Supervisor::wait` blocks until the outbox has drained every queued
    /// record to the log. After this returns, a record enqueued before shutdown
    /// is guaranteed appended — never lost.
    pub async fn shutdown_and_drain(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.supervisor.wait().await;
    }
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
    let agent_endpoint = stub_agent(sse_body).await;
    spawn_core(
        policy,
        ProxyConfig::new(agent_endpoint, "127.0.0.1:0".to_string()),
    )
    .await
}

/// Boot the database and the server wired to `policy` behind a fully-built
/// `proxy` config — the shared core under [`spawn`] / [`spawn_with_policy`], and
/// the entry point for tests that need a non-default proxy config (e.g. session
/// memory enabled, pointing at a multi-response stub agent).
pub async fn spawn_core(policy: Arc<dyn PolicyPort>, proxy: ProxyConfig) -> TestServer {
    spawn_core_with_outbox(
        policy,
        proxy,
        OutboxSettings {
            capacity: 1024,
            on_full: FullPolicy::Block,
        },
    )
    .await
}

/// Like [`spawn_core`] but with an explicit [`OutboxSettings`] — for lifecycle
/// tests that exercise the audit write path under backpressure (a tiny capacity
/// with [`FullPolicy::Shed`], so the outbox sheds rather than blocking the proxy).
pub async fn spawn_core_with_outbox(
    policy: Arc<dyn PolicyPort>,
    proxy: ProxyConfig,
    outbox: OutboxSettings,
) -> TestServer {
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
            outbox,
        },
        &supervisor,
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    // Serve with a graceful-shutdown hook so a test can stop serving on demand:
    // when the signal fires (or its sender drops), `serve` returns, the app drops,
    // and the outbox channel closes — the same drain trigger as production.
    let (shutdown, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });

    TestServer {
        container,
        pool,
        base_url: format!("http://{addr}"),
        log,
        supervisor,
        shutdown: Some(shutdown),
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

/// Boot a stub AG-UI agent that answers each successive `POST /run` with the next
/// body in `bodies`, clamping to the last once exhausted — for multi-run tests
/// where the agent's response must differ across runs (e.g. session-memory
/// replay: a denied tool in run 1, the same tool with clean arguments in run 2).
pub async fn stub_agent_sequence(bodies: Vec<String>) -> String {
    let bodies = Arc::new(bodies);
    let next = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let app = Router::new().route(
        "/run",
        post(move || {
            let bodies = Arc::clone(&bodies);
            let next = Arc::clone(&next);
            async move {
                let index = next
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    .min(bodies.len() - 1);
                ([(CONTENT_TYPE, "text/event-stream")], bodies[index].clone())
            }
        }),
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

/// Drive a [`REDACT_DENY_SSE`] run and assert the shared redact-and-deny outcome:
/// the secret `sk-LEAK` is masked, the tool `rm_rf` is denied (no frames leak),
/// and all four events are recorded. Shared by the static-ruleset and the
/// CEL/Rego plugin-engine e2e so the assertion lives in one place.
pub async fn assert_redacts_secret_and_denies_tool(app: &TestServer) {
    let body = client()
        .post(&app.base_url)
        .body("{}")
        .send()
        .await
        .expect("proxy responds")
        .text()
        .await
        .expect("read streamed body");

    assert!(
        body.contains("RUN_STARTED") && body.contains("RUN_FINISHED"),
        "lifecycle forwarded: {body}"
    );
    assert!(!body.contains("sk-LEAK"), "secret leaked to client: {body}");
    assert!(body.contains("[REDACTED]"), "message was redacted: {body}");
    assert!(!body.contains("rm_rf"), "denied tool leaked: {body}");
    assert!(
        !body.contains("TOOL_CALL"),
        "denied tool frames leaked: {body}"
    );

    let container = audit_container(app.pool.clone());
    let registry = audit_registry();
    let recorded = poll_inclusion(&container, &registry, app.log, LeafIndex(3)).await;
    assert!(
        recorded,
        "all four Ready events recorded (lifecycle x2 + redacted message + denied tool)"
    );
}
