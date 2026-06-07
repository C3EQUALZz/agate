//! Full HTTP end-to-end: drive the booted application over the wire and verify
//! both the responses and the resulting database state.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::fixture::spawn;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_append_and_prove_over_http() {
    let app = spawn().await;
    let client = reqwest::Client::new();

    // Create an empty log.
    let response = client
        .post(format!("{}/logs", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let body: Value = response.json().await.unwrap();
    let log_id = body["id"].as_str().unwrap().to_owned();

    // Append three records, each its own HTTP request (own transaction).
    for (expected_index, record) in ["a", "b", "c"].iter().enumerate() {
        let response = client
            .post(format!("{}/logs/{log_id}/records", app.base_url))
            .json(&json!({ "record": record }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["index"], expected_index as u64);
    }

    // Inclusion proof for leaf 1 over a tree of size 3.
    let response = client
        .get(format!("{}/logs/{log_id}/inclusion/1", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let proof: Value = response.json().await.unwrap();
    assert_eq!(proof["tree_size"], 3);
    assert_eq!(proof["leaf_index"], 1);
    assert!(!proof["path"].as_array().unwrap().is_empty());

    // The records were really committed to the database.
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM audit_leaf WHERE log_id = $1")
        .bind(Uuid::parse_str(&log_id).unwrap())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn checkpoint_without_a_signing_key_fails_cleanly() {
    let app = spawn().await;
    let client = reqwest::Client::new();

    // A log to checkpoint.
    let body: Value = client
        .post(format!("{}/logs", app.base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let log_id = body["id"].as_str().unwrap().to_owned();

    // No AUDIT_CHECKPOINT_SEED is configured in the test process, so signing is
    // disabled: the route is wired and dispatches, but fails cleanly rather than
    // signing under an ephemeral key.
    let response = client
        .post(format!("{}/logs/{log_id}/checkpoint", app.base_url))
        .json(&json!({ "key_id": "checkpoint-ed25519" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 500);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"], "key_not_found");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn append_to_missing_log_is_not_found() {
    let app = spawn().await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/logs/{}/records", app.base_url, Uuid::new_v4()))
        .json(&json!({ "record": "x" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["error"], "log_not_found");
}
