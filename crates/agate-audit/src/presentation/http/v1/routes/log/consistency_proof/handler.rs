use axum::extract::Path;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Json, Router};
use froodi::async_impl::Container;
use uuid::Uuid;

use super::schema::ConsistencyProofResponse;
use crate::application::usecases::get_consistency_proof::GetConsistencyProof;
use crate::domain::merkle::{LogId, TreeSize};
use crate::presentation::http::v1::common::dispatch::SharedRegistry;
use crate::presentation::http::v1::common::{HttpError, dispatcher};

pub fn router() -> Router {
    Router::new().route("/logs/{id}/consistency/{first}", get(consistency_proof))
}

async fn consistency_proof(
    Extension(container): Extension<Container>,
    Extension(registry): Extension<SharedRegistry>,
    Path((id, first)): Path<(Uuid, u64)>,
) -> Result<impl IntoResponse, HttpError> {
    let view = dispatcher(container, registry)
        .send(GetConsistencyProof {
            log: LogId(id),
            first: TreeSize(first),
        })
        .await?;
    Ok(Json(ConsistencyProofResponse::from(view)))
}
