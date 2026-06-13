use std::fmt::Write as _;

use agate_crypto::KeyId;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use froodi::Inject;
use froodi::async_impl::Container;
use uuid::Uuid;

use super::schema::{IssueCheckpointRequest, IssueCheckpointResponse};
use crate::application::common::messaging::Dispatcher;
use crate::application::usecases::issue_checkpoint::IssueCheckpoint;
use crate::domain::merkle::{LogId, SignedTreeHead};
use crate::presentation::http::v1::common::HttpError;

pub fn router() -> Router {
    Router::new().route("/logs/{id}/checkpoint", post(issue_checkpoint))
}

/// Snapshot, sign, and anchor a checkpoint (Signed Tree Head) for the log.
async fn issue_checkpoint(
    Inject(dispatcher): Inject<Dispatcher<Container>>,
    Path(id): Path<Uuid>,
    Json(body): Json<IssueCheckpointRequest>,
) -> Result<impl IntoResponse, HttpError> {
    let sth = dispatcher
        .send(IssueCheckpoint {
            log: LogId(id),
            key: KeyId(body.key_id),
            // The API always issues; idle-skip is for the periodic issuer.
            previous_size: None,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(to_response(&sth))))
}

fn to_response(sth: &SignedTreeHead) -> IssueCheckpointResponse {
    IssueCheckpointResponse {
        size: sth.head.size.value(),
        root: sth.head.root.to_hex(),
        at_ms: sth.head.at.as_millis(),
        key_id: sth.signature.key_id.0.clone(),
        algorithm: format!("{:?}", sth.signature.algo),
        signature: to_hex(&sth.signature.bytes),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use agate_crypto::{Digest, HashAlgo, SignAlgo, Signature};

    use super::{KeyId, SignedTreeHead, to_hex, to_response};
    use crate::domain::common::values::Timestamp;
    use crate::domain::merkle::{TreeHead, TreeSize};

    #[test]
    fn to_hex_lowercases_and_zero_pads() {
        assert_eq!(to_hex(&[0x00, 0x0f, 0xab]), "000fab");
        assert_eq!(to_hex(&[]), "");
    }

    #[test]
    fn to_response_projects_the_signed_tree_head() {
        let sth = SignedTreeHead {
            head: TreeHead {
                size: TreeSize(3),
                root: Digest {
                    algo: HashAlgo::Sha256,
                    bytes: vec![0xab, 0xcd],
                },
                at: Timestamp::from_millis(1234).expect("valid timestamp"),
            },
            signature: Signature {
                algo: SignAlgo::Ed25519,
                key_id: KeyId("checkpoint-ed25519".to_owned()),
                bytes: vec![0x01, 0x02],
            },
        };

        let response = to_response(&sth);
        assert_eq!(response.size, 3);
        assert_eq!(response.root, "abcd");
        assert_eq!(response.at_ms, 1234);
        assert_eq!(response.key_id, "checkpoint-ed25519");
        assert_eq!(response.algorithm, "Ed25519");
        assert_eq!(response.signature, "0102");
    }
}
