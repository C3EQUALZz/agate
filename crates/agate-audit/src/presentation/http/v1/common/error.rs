use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::application::errors::AuditError;

/// Wraps an [`AuditError`] so handlers can `?` it into an HTTP response.
pub struct HttpError(AuditError);

impl From<AuditError> for HttpError {
    fn from(error: AuditError) -> Self {
        Self(error)
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    detail: String,
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            AuditError::LogNotFound(_) => (StatusCode::NOT_FOUND, "log_not_found"),
            AuditError::LeafOutOfRange { .. } | AuditError::SizeOutOfRange { .. } => {
                (StatusCode::BAD_REQUEST, "out_of_range")
            }
            AuditError::KeyNotFound(_) => (StatusCode::INTERNAL_SERVER_ERROR, "key_not_found"),
            AuditError::Domain(_) => (StatusCode::BAD_REQUEST, "domain_error"),
            AuditError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, "storage_error"),
        };
        let body = Json(ErrorBody {
            error: code,
            detail: self.0.to_string(),
        });
        (status, body).into_response()
    }
}
