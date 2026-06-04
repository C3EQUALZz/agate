use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Extension, Json, Router};
use froodi::async_impl::Container;

use super::schema::CreateLogResponse;
use crate::application::usecases::create_log::CreateLog;
use crate::presentation::http::v1::common::dispatch::SharedRegistry;
use crate::presentation::http::v1::common::{HttpError, dispatcher};

pub fn router() -> Router {
    Router::new().route("/logs", post(create_log))
}

async fn create_log(
    Extension(container): Extension<Container>,
    Extension(registry): Extension<SharedRegistry>,
) -> Result<impl IntoResponse, HttpError> {
    let id = dispatcher(container, registry).send(CreateLog).await?;
    Ok((StatusCode::CREATED, Json(CreateLogResponse { id: id.0 })))
}
