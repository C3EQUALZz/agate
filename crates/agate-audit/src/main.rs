//! Runs the audit HTTP service. Configure with `DATABASE_URL` (required) and
//! `BIND_ADDR` (default `0.0.0.0:8080`).

use sqlx::postgres::PgPoolOptions;

use agate_audit::infrastructure::persistence::postgres::run_migrations;
use agate_audit::presentation::build_app;

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .expect("connect to Postgres");
    run_migrations(&pool).await.expect("run migrations");

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("bind listener");
    axum::serve(listener, build_app(pool)).await.expect("serve");
}
