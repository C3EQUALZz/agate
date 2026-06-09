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
/// broke mid-response. The variant is the failure *kind*, so callers can react
/// per kind (status mapping, the metric label) instead of parsing a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpstreamError {
    /// Could not establish a connection (DNS, refused, TLS).
    Connect(String),
    /// The connect or between-chunk read deadline elapsed.
    Timeout,
    /// The agent answered with a non-success HTTP status.
    Status(u16),
    /// The response stream broke or yielded an invalid frame.
    Stream(String),
}

impl UpstreamError {
    /// The stable metric-label value for this failure kind — the `kind` label
    /// on the `agate_upstream_errors_total` counter.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Connect(_) => "connect",
            Self::Timeout => "timeout",
            Self::Status(_) => "status",
            Self::Stream(_) => "stream",
        }
    }
}

impl fmt::Display for UpstreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(message) => {
                write!(f, "could not connect to the upstream agent: {message}")
            }
            Self::Timeout => write!(f, "the upstream agent timed out"),
            Self::Status(code) => write!(f, "the upstream agent answered HTTP {code}"),
            Self::Stream(message) => write!(f, "the upstream response stream failed: {message}"),
        }
    }
}

impl std::error::Error for UpstreamError {}
