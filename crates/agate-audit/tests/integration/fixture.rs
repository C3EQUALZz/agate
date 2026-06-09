//! Shared infrastructure fixture: a real PostgreSQL via testcontainers, with
//! migrations applied.

#![allow(dead_code)]

use sqlx::PgPool;
use testcontainers::ContainerAsync;
use testcontainers::ImageExt;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

/// Pinned Postgres image — a current LTS, not testcontainers' EOL default.
const POSTGRES_IMAGE_TAG: &str = "17-alpine";

use agate_audit::infrastructure::persistence::postgres::{
    PoolConfig, connect_pool, run_migrations,
};

/// A running PostgreSQL container with migrations applied; holds the container
/// alive (RAII) and exposes a connected pool and its connection URL.
pub struct Db {
    pub container: ContainerAsync<Postgres>,
    pub pool: PgPool,
    pub url: String,
}

pub async fn start() -> Db {
    let container = Postgres::default()
        .with_tag(POSTGRES_IMAGE_TAG)
        .start()
        .await
        .unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = connect_pool(&url, &PoolConfig::default()).await.unwrap();
    run_migrations(&pool).await.unwrap();
    Db {
        container,
        pool,
        url,
    }
}
