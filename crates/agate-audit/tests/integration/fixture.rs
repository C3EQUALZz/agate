//! Shared infrastructure fixture: a real PostgreSQL via testcontainers, with
//! migrations applied.

#![allow(dead_code)]

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

use agate_audit::infrastructure::persistence::postgres::run_migrations;

/// A running PostgreSQL container with migrations applied; holds the container
/// alive (RAII) and exposes a connected pool.
pub struct Db {
    pub container: ContainerAsync<Postgres>,
    pub pool: PgPool,
}

pub async fn start() -> Db {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = PgPoolOptions::new().connect(&url).await.unwrap();
    run_migrations(&pool).await.unwrap();
    Db { container, pool }
}
