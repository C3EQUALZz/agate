//! Runs the audit HTTP service. Configure with `DATABASE_URL` (required) and
//! `BIND_ADDR` (default `0.0.0.0:8080`).

use sqlx::postgres::PgPoolOptions;

use agate_audit::infrastructure::persistence::postgres::run_migrations;
use agate_audit::setup::bootstrap::build_app;
use agate_audit::setup::configs::AppConfig;

#[tokio::main]
async fn main() {
    let config = AppConfig::from_env();

    let pool = PgPoolOptions::new()
        .connect(config.postgres.url())
        .await
        .expect("connect to Postgres");
    run_migrations(&pool).await.expect("run migrations");

    let listener = tokio::net::TcpListener::bind(&config.http.bind_addr)
        .await
        .expect("bind listener");
    axum::serve(listener, build_app(pool)).await.expect("serve");
}
