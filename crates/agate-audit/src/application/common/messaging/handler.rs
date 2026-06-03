use async_trait::async_trait;

use super::request::Request;

/// Handles exactly one [`Request`] type.
#[async_trait]
pub trait RequestHandler<R: Request>: Send + Sync {
    async fn handle(&self, request: R) -> R::Response;
}
