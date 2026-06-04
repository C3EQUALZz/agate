use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Extension, Json, Router};
use froodi::async_impl::Container;
use uuid::Uuid;

use super::schema::{AppendRecordRequest, AppendRecordResponse};
use crate::application::usecases::append_record::AppendRecord;
use crate::domain::merkle::LogId;
use crate::presentation::http::v1::common::dispatch::SharedRegistry;
use crate::presentation::http::v1::common::{HttpError, dispatcher};

pub fn router() -> Router {
    Router::new().route("/logs/{id}/records", post(append_record))
}

async fn append_record(
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
