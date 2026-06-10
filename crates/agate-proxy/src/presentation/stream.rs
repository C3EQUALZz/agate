use std::sync::Arc;

use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{Value, json};
use tracing::{debug, info, warn};

use crate::application::common::ports::{AgentResponseStream, InspectionOutcome, ProxyMetrics};
use crate::application::inspection::{
    InspectionAction, InspectionContext, Inspector, MalformedEventMode,
};
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
/// markers, unknown types, non-objects, non-JSON) pass through unchanged.
///
/// A **recognized but malformed** event (a known `type` with a missing/blank
/// required field) cannot be inspected yet belongs to the run, so forwarding it
/// raw would bypass the policy. `malformed_mode` decides its fate
/// ([`MalformedEventMode`]); the default is to fail closed and terminate.
pub fn inspect_stream(
    mut upstream: AgentResponseStream,
    inspector: Arc<Inspector>,
    context: InspectionContext,
    budgets: Budgets,
    malformed_mode: MalformedEventMode,
    metrics: Arc<dyn ProxyMetrics>,
) -> impl Stream<Item = Bytes> + Send {
    async_stream::stream! {
        let mut decoder = SseDecoder::new();
        let mut run = Run::new(context.run, budgets);
        let mut pending: Vec<String> = Vec::new();

        while let Some(chunk) = upstream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    warn!(run = %context.run, %error, "upstream stream error; ending run with RUN_ERROR");
                    metrics.record_upstream_error(&error);
                    yield Bytes::from(run_error(&error.to_string()));
                    return;
                }
            };

            for event in decoder.push(&chunk) {
                let fragment = match serde_json::from_str::<Value>(&event.data) {
                    Ok(value) => match to_fragment(&value) {
                        Ok(fragment) => fragment,
                        // Recognized event type, but a required field is missing
                        // or blank: it cannot be inspected, so it must not slip
                        // past the policy. Fail closed per the configured mode.
                        Err(error) if error.is_malformed_known() => match malformed_mode {
                            MalformedEventMode::Forward => None,
                            MalformedEventMode::Drop => {
                                warn!(run = %context.run, %error, "dropping a malformed known event");
                                metrics.record_inspected(InspectionOutcome::Deny);
                                continue;
                            }
                            MalformedEventMode::Terminate => {
                                warn!(run = %context.run, %error, "terminating run on a malformed known event");
                                metrics.record_inspected(InspectionOutcome::Terminate);
                                pending.clear();
                                yield Bytes::from(run_error("malformed protocol event"));
                                return;
                            }
                        },
                        // Not an object / no `type` → not an AG-UI event we
                        // inspect; forward like any uninspectable frame.
                        Err(_) => None,
                    },
                    // Not JSON → not inspectable.
                    Err(_) => None,
                };

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
                        debug!(run = %context.run, "forwarding inspected event");
                        metrics.record_inspected(InspectionOutcome::Forward);
                        for held in pending.drain(..) {
                            yield Bytes::from(held);
                        }
                        yield Bytes::from(event.raw);
                    }
                    InspectionAction::Hold => {
                        debug!(run = %context.run, "buffering event until the tool call is complete");
                        metrics.record_inspected(InspectionOutcome::Buffer);
                        pending.push(event.raw);
                    }
                    InspectionAction::ForwardTransformed(replacement) => {
                        info!(run = %context.run, "policy transformed an event (e.g. redaction); forwarding the replacement");
                        metrics.record_inspected(InspectionOutcome::Transform);
                        pending.clear();
                        match to_event(&replacement) {
                            Some(value) => yield Bytes::from(encode(&value.to_string())),
                            None => yield Bytes::from(event.raw),
                        }
                    }
                    InspectionAction::Drop(reason) => {
                        info!(run = %context.run, reason = reason.as_str(), "policy denied an event; dropping it");
                        metrics.record_inspected(InspectionOutcome::Deny);
                        pending.clear();
                    }
                    InspectionAction::Terminate(reason) => {
                        warn!(run = %context.run, reason = reason.as_str(), "terminating run with RUN_ERROR");
                        metrics.record_inspected(InspectionOutcome::Terminate);
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

    /// Replaces every message chunk with a fixed redacted text.
    struct RedactMessages;
    #[async_trait]
    impl PolicyPort for RedactMessages {
        async fn decide(&self, _: &InspectionContext, event: &AgentEvent) -> Verdict<AgentEvent> {
            match event {
                AgentEvent::MessageChunk { message, .. } => {
                    Verdict::Transform(AgentEvent::MessageChunk {
                        message: message.clone(),
                        text: "[redacted]".into(),
                    })
                }
                _ => Verdict::Allow,
            }
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
        InspectionContext::new(SessionId::new(Uuid::nil()), RunId::new(Uuid::nil()))
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

    /// A fake [`ProxyMetrics`] that records every call, for asserting outcomes.
    #[derive(Default)]
    struct CountingMetrics {
        upstream_errors: std::sync::atomic::AtomicUsize,
        inspected: std::sync::Mutex<Vec<InspectionOutcome>>,
    }

    impl ProxyMetrics for CountingMetrics {
        fn record_run(&self) {}
        fn record_upstream_error(&self, _: &UpstreamError) {
            self.upstream_errors
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        fn record_inspected(&self, outcome: InspectionOutcome) {
            self.inspected.lock().unwrap().push(outcome);
        }
    }

    fn metrics() -> Arc<dyn ProxyMetrics> {
        Arc::new(CountingMetrics::default())
    }

    /// Run `inspect_stream` with the default budgets/context and a fail-closed
    /// malformed mode — the shape every test below shares.
    fn inspect(
        upstream: AgentResponseStream,
        inspector: Arc<Inspector>,
        metrics: Arc<dyn ProxyMetrics>,
    ) -> impl Stream<Item = Bytes> + Send {
        inspect_stream(
            upstream,
            inspector,
            context(),
            Budgets::default(),
            MalformedEventMode::Terminate,
            metrics,
        )
    }

    #[tokio::test]
    async fn transforms_a_message_and_forwards_the_replacement() {
        let stream = inspect(
            upstream(&[
                "data: {\"type\":\"RUN_STARTED\"}\n\n",
                "data: {\"type\":\"TEXT_MESSAGE_CONTENT\",\"messageId\":\"m1\",\"delta\":\"secret\"}\n\n",
                "data: {\"type\":\"RUN_FINISHED\"}\n\n",
            ]),
            inspector(Arc::new(RedactMessages)),
            metrics(),
        );

        let out = collect(stream).await;
        assert!(
            out.contains("[redacted]"),
            "expected the replacement: {out}"
        );
        assert!(
            !out.contains("secret"),
            "original text should be gone: {out}"
        );
    }

    #[tokio::test]
    async fn forwards_an_allowed_run() {
        let stream = inspect(
            upstream(&[
                "data: {\"type\":\"RUN_STARTED\"}\n\n",
                "data: {\"type\":\"RUN_FINISHED\"}\n\n",
            ]),
            inspector(Arc::new(AllowAllPolicy)),
            metrics(),
        );

        let out = collect(stream).await;
        assert!(out.contains("RUN_STARTED"));
        assert!(out.contains("RUN_FINISHED"));
    }

    #[tokio::test]
    async fn buffers_a_tool_call_and_forwards_it_on_end() {
        let stream = inspect(
            upstream(&[
                "data: {\"type\":\"RUN_STARTED\"}\n\n",
                "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\",\"toolCallName\":\"x\"}\n\n",
                "data: {\"type\":\"TOOL_CALL_ARGS\",\"toolCallId\":\"c1\",\"delta\":\"{}\"}\n\n",
                "data: {\"type\":\"TOOL_CALL_END\",\"toolCallId\":\"c1\"}\n\n",
            ]),
            inspector(Arc::new(AllowAllPolicy)),
            metrics(),
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
        let stream = inspect(
            upstream(&["data: {\"type\":\"RUN_STARTED\"}\n\n"]),
            inspector(Arc::new(DenyAll)),
            metrics(),
        );

        let out = collect(stream).await;
        assert!(!out.contains("RUN_STARTED"));
    }

    #[tokio::test]
    async fn upstream_error_ends_with_a_run_error() {
        let upstream = stream::iter(vec![Err(UpstreamError::Stream("boom".to_string()))]).boxed();
        let stream = inspect(upstream, inspector(Arc::new(AllowAllPolicy)), metrics());

        let out = collect(stream).await;
        assert!(out.contains("RUN_ERROR"));
        assert!(out.contains("boom"));
    }

    #[tokio::test]
    async fn records_the_inspection_outcome_through_the_port() {
        let recorder = Arc::new(CountingMetrics::default());
        let metrics: Arc<dyn ProxyMetrics> = recorder.clone();
        let stream = inspect(
            upstream(&["data: {\"type\":\"RUN_STARTED\"}\n\n"]),
            inspector(Arc::new(DenyAll)),
            metrics,
        );

        let _ = collect(stream).await;
        assert_eq!(
            *recorder.inspected.lock().unwrap(),
            vec![InspectionOutcome::Deny],
        );
    }

    /// A recognized event whose required field is missing must not slip past
    /// the policy: by default it fails closed and terminates the run.
    #[tokio::test]
    async fn a_malformed_known_event_terminates_by_default() {
        let stream = inspect(
            upstream(&[
                "data: {\"type\":\"RUN_STARTED\"}\n\n",
                // TOOL_CALL_START with no toolCallName — recognized but malformed.
                "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\"}\n\n",
                "data: {\"type\":\"RUN_FINISHED\"}\n\n",
            ]),
            inspector(Arc::new(AllowAllPolicy)),
            metrics(),
        );

        let out = collect(stream).await;
        assert!(out.contains("RUN_ERROR"), "the run is terminated: {out}");
        assert!(
            !out.contains("TOOL_CALL_START"),
            "the malformed frame never reaches the client: {out}"
        );
        assert!(
            !out.contains("RUN_FINISHED"),
            "the stream ends at the malformed event: {out}"
        );
    }

    /// In `Forward` mode the same malformed frame is forwarded raw (the legacy,
    /// availability-over-safety behavior).
    #[tokio::test]
    async fn a_malformed_known_event_is_forwarded_in_forward_mode() {
        let stream = inspect_stream(
            upstream(&[
                "data: {\"type\":\"RUN_STARTED\"}\n\n",
                "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\"}\n\n",
            ]),
            inspector(Arc::new(AllowAllPolicy)),
            context(),
            Budgets::default(),
            MalformedEventMode::Forward,
            metrics(),
        );

        let out = collect(stream).await;
        assert!(
            out.contains("TOOL_CALL_START"),
            "forward mode passes the raw frame through: {out}"
        );
    }

    /// An unrecognized (future) event type carries nothing to inspect and is
    /// forwarded unchanged even under the fail-closed default.
    #[tokio::test]
    async fn an_unknown_event_type_is_forwarded() {
        let stream = inspect(
            upstream(&[
                "data: {\"type\":\"RUN_STARTED\"}\n\n",
                "data: {\"type\":\"SOME_FUTURE_EVENT\",\"x\":1}\n\n",
            ]),
            inspector(Arc::new(AllowAllPolicy)),
            metrics(),
        );

        let out = collect(stream).await;
        assert!(
            out.contains("SOME_FUTURE_EVENT"),
            "an unknown type is not treated as malformed: {out}"
        );
    }
}
