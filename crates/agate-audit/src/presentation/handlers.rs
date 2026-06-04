use std::sync::Arc;

use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use froodi::async_impl::Container;
use uuid::Uuid;

use super::dto::{
    AppendRecordRequest, AppendRecordResponse, ConsistencyProofResponse, CreateLogResponse,
    InclusionProofResponse,
};
use super::error::HttpError;
use crate::application::common::messaging::{Dispatcher, Registry};
use crate::application::usecases::append_record::AppendRecord;
use crate::application::usecases::create_log::CreateLog;
use crate::application::usecases::get_consistency_proof::GetConsistencyProof;
use crate::application::usecases::get_inclusion_proof::GetInclusionProof;
use crate::domain::merkle::{LeafIndex, LogId, TreeSize};

/// The routing table, shared as an axum extension across requests.
type SharedRegistry = Arc<Registry<Container>>;

/// Build a dispatcher over this request's froodi scope (its own transaction).
fn dispatcher(container: Container, registry: SharedRegistry) -> Dispatcher<Container> {
    Dispatcher::new(Arc::new(container), registry)
}

pub async fn create_log(
    Extension(container): Extension<Container>,
    Extension(registry): Extension<SharedRegistry>,
) -> Result<impl IntoResponse, HttpError> {
    let id = dispatcher(container, registry).send(CreateLog).await?;
    Ok((StatusCode::CREATED, Json(CreateLogResponse { id: id.0 })))
}

pub async fn append_record(
    Extension(container): Extension<Container>,
    Extension(registry): Extension<SharedRegistry>,
    Path(id): Path<Uuid>,
    Json(body): Json<AppendRecordRequest>,
) -> Result<impl IntoResponse, HttpError> {
    let index = dispatcher(container, registry)
        .send(AppendRecord {
            log: LogId(id),
            record: body.record.into_bytes(),
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(AppendRecordResponse { index: index.0 }),
    ))
}

pub async fn inclusion_proof(
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

pub async fn consistency_proof(
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
