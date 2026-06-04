use serde::Serialize;
use uuid::Uuid;

/// Identifier assigned to a newly-created transparency log.
#[derive(Serialize)]
pub struct CreateLogResponse {
    pub id: Uuid,
}
