use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use froodi::Inject;
use froodi::async_impl::Container;

use super::schema::CreateLogResponse;
use crate::application::common::messaging::Dispatcher;
use crate::application::usecases::create_log::CreateLog;
use crate::presentation::http::v1::common::HttpError;

pub fn router() -> Router {
    Router::new().route("/logs", post(create_log))
}

async fn create_log(
    Inject(dispatcher): Inject<Dispatcher<Container>>,
) -> Result<impl IntoResponse, HttpError> {
    let id = dispatcher.send(CreateLog).await?;
    Ok((StatusCode::CREATED, Json(CreateLogResponse { id: id.0 })))
}
