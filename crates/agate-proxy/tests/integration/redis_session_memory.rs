//! The Redis session-memory backend against a real Redis (testcontainers),
//! plus the fail-open contract when Redis is unreachable.

use std::time::Duration;

use agate_proxy::application::common::ports::SessionMemory;
use agate_proxy::domain::inspection::{DenyReason, SessionId};
use agate_proxy::infrastructure::RedisSessionMemory;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;
use uuid::Uuid;

fn session() -> SessionId {
    SessionId::new(Uuid::nil())
}

fn reason() -> DenyReason {
    DenyReason::new("tool not allowed")
}

#[tokio::test]
async fn remembers_and_recalls_a_denied_tool_across_the_shared_store() {
    let container = Redis::default().start().await.expect("start redis");
    let port = container
        .get_host_port_ipv4(6379)
        .await
        .expect("redis port");
    let memory = RedisSessionMemory::new(
        &format!("redis://127.0.0.1:{port}"),
        Duration::from_hours(1),
    )
    .expect("valid url");

    // A fresh session knows nothing.
    assert!(memory.recall(session(), "delete_file").await.is_none());

    // A denial is remembered and recalled within the session...
    memory.remember(session(), "delete_file", &reason()).await;
    assert_eq!(
        memory.recall(session(), "delete_file").await,
        Some(reason())
    );
    // ...but a different tool is not quarantined, and a different session is
    // isolated (the store is keyed by session id).
    assert!(memory.recall(session(), "search").await.is_none());
    let other = SessionId::new(Uuid::from_u128(1));
    assert!(memory.recall(other, "delete_file").await.is_none());
}

#[tokio::test]
async fn keeps_the_first_denial_reason() {
    let container = Redis::default().start().await.expect("start redis");
    let port = container
        .get_host_port_ipv4(6379)
        .await
        .expect("redis port");
    let memory = RedisSessionMemory::new(
        &format!("redis://127.0.0.1:{port}"),
        Duration::from_hours(1),
    )
    .expect("valid url");

    memory
        .remember(session(), "fetch", &DenyReason::new("first"))
        .await;
    memory
        .remember(session(), "fetch", &DenyReason::new("second"))
        .await;
    assert_eq!(
        memory.recall(session(), "fetch").await,
        Some(DenyReason::new("first"))
    );
}

#[tokio::test]
async fn an_unreachable_redis_fails_open_to_no_memory() {
    // Port 1 is not listening: every operation must degrade silently — recall
    // returns None and remember is a no-op, never an error or a wrong allow.
    let memory =
        RedisSessionMemory::new("redis://127.0.0.1:1", Duration::from_hours(1)).expect("valid url");

    memory.remember(session(), "delete_file", &reason()).await;
    assert!(
        memory.recall(session(), "delete_file").await.is_none(),
        "an unreachable Redis must not quarantine (fail-open over the stateless policy)"
    );
}
