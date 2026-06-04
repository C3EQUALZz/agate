use std::fmt;

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;

/// A run to forward to the upstream agent: the raw request body (an AG-UI
/// `RunAgentInput`) and the headers to pass through (e.g. authorization).
pub struct RunRequest {
    pub body: Bytes,
    pub headers: Vec<(String, String)>,
}

/// The agent's streaming response as raw byte chunks (the SSE body), fed into
/// the [`SseDecoder`](crate::infrastructure::sse::SseDecoder).
pub type AgentResponseStream = BoxStream<'static, Result<Bytes, UpstreamError>>;

/// Forwards a run to the upstream agent and returns its streaming response.
#[async_trait]
pub trait UpstreamAgentClient: Send + Sync {
    async fn run(&self, request: RunRequest) -> Result<AgentResponseStream, UpstreamError>;
}

/// The upstream agent was unreachable, rejected the request, or the stream
/// broke mid-response.
#[derive(Debug, Clone)]
pub struct UpstreamError(pub String);

impl fmt::Display for UpstreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "upstream agent error: {}", self.0)
    }
}

impl std::error::Error for UpstreamError {}
