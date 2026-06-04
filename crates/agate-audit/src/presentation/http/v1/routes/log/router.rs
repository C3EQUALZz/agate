use axum::Router;

use super::{append_record, consistency_proof, create, inclusion_proof};

/// Aggregate the transparency-log operation routes (each owns its full path).
pub fn router() -> Router {
    Router::new()
        .merge(create::router())
        .merge(append_record::router())
        .merge(inclusion_proof::router())
        .merge(consistency_proof::router())
}
