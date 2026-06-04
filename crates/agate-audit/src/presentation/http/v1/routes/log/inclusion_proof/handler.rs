use axum::extract::Path;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Json, Router};
use froodi::async_impl::Container;
use uuid::Uuid;

use super::schema::InclusionProofResponse;
use crate::application::usecases::get_inclusion_proof::GetInclusionProof;
use crate::domain::merkle::{LeafIndex, LogId};
use crate::presentation::http::v1::common::dispatch::SharedRegistry;
use crate::presentation::http::v1::common::{HttpError, dispatcher};

pub fn router() -> Router {
    Router::new().route("/logs/{id}/inclusion/{index}", get(inclusion_proof))
}

async fn inclusion_proof(
    Extension(container): Extension<Container>,
    Extension(registry): Extension<SharedRegistry>,
    Path((id, index)): Path<(Uuid, u64)>,
) -> Result<impl IntoResponse, HttpError> {
    let view = dispatcher(container, registry)
        .send(GetInclusionProof {
            log: LogId(id),
            index: LeafIndex(index),
        })
        .await?;
    Ok(Json(InclusionProofResponse::from(view)))
}
