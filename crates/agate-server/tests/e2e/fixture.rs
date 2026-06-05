//! End-to-end fixture: a real PostgreSQL (testcontainers), a stub AG-UI agent,
//! and the full server (proxy + audit outbox) booted on an ephemeral port.

#![allow(dead_code)]

use std::sync::Arc;

use axum::Router;
use axum::http::header::CONTENT_TYPE;
use axum::routing::post;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use tokio::net::TcpListener;

use agate_audit::application::common::messaging::{Dispatcher, Registry, Request};
use agate_audit::application::usecases::create_log::CreateLog;
use agate_audit::domain::merkle::LogId;
use agate_audit::infrastructure::persistence::postgres::run_migrations;
use agate_audit::setup::ioc::{build_container, build_registry};
use agate_proxy::setup::configs::ProxyConfig;
use agate_server::setup::bootstrap::{Server, build_server};
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

/// Boot the stub agent, the database, and the server; create a fresh log.
pub async fn spawn() -> TestServer {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = PgPoolOptions::new().connect(&url).await.unwrap();
    run_migrations(&pool).await.unwrap();

    let registry = Arc::new(build_registry());
    let log = dispatch(&audit_container(pool.clone()), &registry, CreateLog)
        .await
        .unwrap();

    let agent_endpoint = stub_agent().await;
    let proxy = ProxyConfig::new(agent_endpoint, "127.0.0.1:0".to_string());
    // The outbox task is detached: it lives as long as the served app (and the
    // audit sink inside it) keeps the channel open.
    let Server { app, .. } = build_server(proxy, pool.clone(), log);

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

/// Boot a stub AG-UI agent answering `POST /run` with [`SSE_BODY`].
async fn stub_agent() -> String {
    let app = Router::new().route(
        "/run",
        post(|| async { ([(CONTENT_TYPE, "text/event-stream")], SSE_BODY) }),
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
    build_container(pool)
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
