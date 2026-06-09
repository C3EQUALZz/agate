use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use tracing::debug;

use crate::application::common::ports::{
    AgentResponseStream, RunRequest, UpstreamAgentClient, UpstreamError,
};

/// [`UpstreamAgentClient`] over reqwest: POSTs the run to the agent's endpoint
/// and streams the SSE response back chunk by chunk.
pub struct ReqwestAgentClient {
    client: Client,
    endpoint: String,
}

impl ReqwestAgentClient {
    #[must_use]
    pub fn new(endpoint: String) -> Self {
        Self {
            client: Client::new(),
            endpoint,
        }
    }

    #[must_use]
    pub fn with_client(client: Client, endpoint: String) -> Self {
        Self { client, endpoint }
    }
}

#[async_trait]
impl UpstreamAgentClient for ReqwestAgentClient {
    async fn run(&self, request: RunRequest) -> Result<AgentResponseStream, UpstreamError> {
        debug!(endpoint = %self.endpoint, "POSTing run to upstream agent");
        let mut builder = self.client.post(&self.endpoint).body(request.body);
        for (name, value) in request.headers {
            builder = builder.header(name, value);
        }

        let response = builder
            .send()
            .await
            .map_err(|error| request_error(&error))?;
        let response = response
            .error_for_status()
            .map_err(|error| match error.status() {
                Some(status) => UpstreamError::Status(status.as_u16()),
                None => UpstreamError::Stream(error.to_string()),
            })?;

        let stream = response
            .bytes_stream()
            .map(|chunk| chunk.map_err(|error| chunk_error(&error)));
        Ok(stream.boxed())
    }
}

/// Classify a failure of the request leg (before any response arrived).
fn request_error(error: &reqwest::Error) -> UpstreamError {
    if error.is_timeout() {
        UpstreamError::Timeout
    } else if error.is_connect() {
        UpstreamError::Connect(error.to_string())
    } else {
        UpstreamError::Stream(error.to_string())
    }
}

/// Classify a failure between response chunks (the stream already started).
fn chunk_error(error: &reqwest::Error) -> UpstreamError {
    if error.is_timeout() {
        UpstreamError::Timeout
    } else {
        UpstreamError::Stream(error.to_string())
    }
}
