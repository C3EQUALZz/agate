use std::sync::Arc;

use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{Value, json};

use crate::application::common::ports::AgentResponseStream;
use crate::application::inspection::{InspectionAction, InspectionContext, Inspector};
use crate::domain::inspection::{Budgets, Run};
use crate::infrastructure::ag_ui::{to_event, to_fragment};
use crate::infrastructure::sse::{SseDecoder, encode};

/// Stream the agent's SSE response through inspection, yielding the bytes to
/// forward to the client.
///
/// Each event is decoded ([`SseDecoder`]), mapped to a domain fragment
/// ([`to_fragment`]), and judged by the [`Inspector`]: an allowed event
/// forwards byte-for-byte (after flushing any held frames), a transformed one
/// is re-encoded, a dropped one vanishes, and a terminate (or upstream error)
/// ends the stream with a `RUN_ERROR`. Events that are not inspectable (framing
/// markers, unknown types, non-JSON) pass through unchanged.
pub fn inspect_stream(
    mut upstream: AgentResponseStream,
    inspector: Arc<Inspector>,
    context: InspectionContext,
    budgets: Budgets,
) -> impl Stream<Item = Bytes> + Send {
    async_stream::stream! {
        let mut decoder = SseDecoder::new();
        let mut run = Run::new(context.run, budgets);
        let mut pending: Vec<String> = Vec::new();

        while let Some(chunk) = upstream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    yield Bytes::from(run_error(&error.to_string()));
                    return;
                }
            };

            for event in decoder.push(&chunk) {
                let fragment = serde_json::from_str::<Value>(&event.data)
                    .ok()
                    .and_then(|value| to_fragment(&value).ok().flatten());

                let Some(fragment) = fragment else {
                    // not inspectable: forward (preserving order during a hold)
                    if pending.is_empty() {
                        yield Bytes::from(event.raw);
                    } else {
                        pending.push(event.raw);
                    }
                    continue;
                };

                match inspector.inspect(&mut run, &context, fragment).await {
                    InspectionAction::Forward => {
                        for held in pending.drain(..) {
                            yield Bytes::from(held);
                        }
                        yield Bytes::from(event.raw);
                    }
                    InspectionAction::Hold => pending.push(event.raw),
                    InspectionAction::ForwardTransformed(replacement) => {
                        pending.clear();
                        match to_event(&replacement) {
                            Some(value) => yield Bytes::from(encode(&value.to_string())),
                            None => yield Bytes::from(event.raw),
                        }
                    }
                    InspectionAction::Drop(_) => pending.clear(),
                    InspectionAction::Terminate(reason) => {
                        pending.clear();
                        yield Bytes::from(run_error(reason.as_str()));
                        return;
                    }
                }
            }
        }

        // flush anything still held when the stream ends (best-effort)
        for held in pending.drain(..) {
            yield Bytes::from(held);
        }
    }
}

fn run_error(message: &str) -> String {
    encode(&json!({ "type": "RUN_ERROR", "message": message }).to_string())
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use futures::stream;
    use uuid::Uuid;

    use super::*;
    use crate::application::common::ports::{AuditSink, PolicyPort, UpstreamError};
    use crate::domain::inspection::{AgentEvent, DenyReason, RunId, SessionId, Verdict};
    use crate::infrastructure::AllowAllPolicy;

    struct NoopAudit;
    #[async_trait]
    impl AuditSink for NoopAudit {
        async fn record(&self, _: &InspectionContext, _: &AgentEvent, _: &Verdict<AgentEvent>) {}
    }

    struct DenyAll;
    #[async_trait]
    impl PolicyPort for DenyAll {
        async fn decide(&self, _: &InspectionContext, _: &AgentEvent) -> Verdict<AgentEvent> {
            Verdict::Deny(DenyReason::new("blocked"))
        }
    }

    fn upstream(chunks: &[&'static str]) -> AgentResponseStream {
        let items: Vec<_> = chunks
            .iter()
            .map(|chunk| Ok::<_, UpstreamError>(Bytes::from(*chunk)))
            .collect();
        stream::iter(items).boxed()
    }

    fn context() -> InspectionContext {
        InspectionContext::new(SessionId(Uuid::nil()), RunId(Uuid::nil()))
    }

    async fn collect(stream: impl Stream<Item = Bytes>) -> String {
        let chunks: Vec<Bytes> = stream.collect().await;
        chunks
            .iter()
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
            .collect()
    }

    fn inspector(policy: Arc<dyn PolicyPort>) -> Arc<Inspector> {
        Arc::new(Inspector::new(policy, Arc::new(NoopAudit)))
    }

    #[tokio::test]
    async fn forwards_an_allowed_run() {
        let stream = inspect_stream(
            upstream(&[
                "data: {\"type\":\"RUN_STARTED\"}\n\n",
                "data: {\"type\":\"RUN_FINISHED\"}\n\n",
            ]),
            inspector(Arc::new(AllowAllPolicy)),
            context(),
            Budgets::default(),
        );

        let out = collect(stream).await;
        assert!(out.contains("RUN_STARTED"));
        assert!(out.contains("RUN_FINISHED"));
    }

    #[tokio::test]
    async fn buffers_a_tool_call_and_forwards_it_on_end() {
        let stream = inspect_stream(
            upstream(&[
                "data: {\"type\":\"RUN_STARTED\"}\n\n",
                "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\",\"toolCallName\":\"x\"}\n\n",
                "data: {\"type\":\"TOOL_CALL_ARGS\",\"toolCallId\":\"c1\",\"delta\":\"{}\"}\n\n",
                "data: {\"type\":\"TOOL_CALL_END\",\"toolCallId\":\"c1\"}\n\n",
            ]),
            inspector(Arc::new(AllowAllPolicy)),
            context(),
            Budgets::default(),
        );

        let out = collect(stream).await;
        // The buffered START/ARGS are flushed together with END, after RUN_STARTED.
        let started = out.find("RUN_STARTED").unwrap();
        let call_start = out.find("TOOL_CALL_START").unwrap();
        let call_end = out.find("TOOL_CALL_END").unwrap();
        assert!(started < call_start && call_start < call_end);
    }

    #[tokio::test]
    async fn denied_events_are_dropped() {
        let stream = inspect_stream(
            upstream(&["data: {\"type\":\"RUN_STARTED\"}\n\n"]),
            inspector(Arc::new(DenyAll)),
            context(),
            Budgets::default(),
        );

        let out = collect(stream).await;
        assert!(!out.contains("RUN_STARTED"));
    }

    #[tokio::test]
    async fn upstream_error_ends_with_a_run_error() {
        let upstream = stream::iter(vec![Err(UpstreamError("boom".to_string()))]).boxed();
        let stream = inspect_stream(
            upstream,
            inspector(Arc::new(AllowAllPolicy)),
            context(),
            Budgets::default(),
        );

        let out = collect(stream).await;
        assert!(out.contains("RUN_ERROR"));
        assert!(out.contains("boom"));
    }
}
