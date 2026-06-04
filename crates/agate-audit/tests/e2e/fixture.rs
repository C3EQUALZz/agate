//! End-to-end fixture: boots the full HTTP application (axum + the froodi
//! composition root) against a real PostgreSQL, on an ephemeral port.

#![allow(dead_code)]

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use tokio::net::TcpListener;

use agate_audit::infrastructure::persistence::postgres::run_migrations;
use agate_audit::setup::bootstrap::build_app;

/// A running application: the HTTP server (background task), its base URL, and a
/// pool to inspect the database directly. Holds the container alive (RAII).
pub struct TestApp {
    pub container: ContainerAsync<Postgres>,
    pub pool: PgPool,
    pub base_url: String,
}

pub async fn spawn() -> TestApp {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = PgPoolOptions::new().connect(&url).await.unwrap();
    run_migrations(&pool).await.unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = build_app(pool.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    TestApp {
        container,
        pool,
        base_url: format!("http://{addr}"),
    }
}
