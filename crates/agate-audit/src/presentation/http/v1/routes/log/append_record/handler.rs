use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use froodi::Inject;
use froodi::async_impl::Container;
use uuid::Uuid;

use super::schema::{AppendRecordRequest, AppendRecordResponse};
use crate::application::common::messaging::Dispatcher;
use crate::application::usecases::append_record::AppendRecord;
use crate::domain::merkle::LogId;
use crate::presentation::http::v1::common::HttpError;

pub fn router() -> Router {
    Router::new().route("/logs/{id}/records", post(append_record))
}

async fn append_record(
    Inject(dispatcher): Inject<Dispatcher<Container>>,
    Path(id): Path<Uuid>,
    Json(body): Json<AppendRecordRequest>,
) -> Result<impl IntoResponse, HttpError> {
    let index = dispatcher
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
