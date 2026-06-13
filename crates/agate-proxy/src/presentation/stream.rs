use std::sync::Arc;

use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::{Value, json};
use tracing::{debug, info, warn};

use crate::application::common::ports::{AgentResponseStream, InspectionOutcome, ProxyMetrics};
use crate::application::inspection::{
    InspectionAction, InspectionContext, InspectionSettings, Inspector, MalformedEventMode,
};
use crate::domain::inspection::{Fragment, Run};
use crate::infrastructure::ag_ui::{AgUiError, to_event, to_fragment};
use crate::infrastructure::sse::{SseDecoder, encode};

/// What one decoded SSE event resolves to before inspection — the classification
/// step kept separate from the streaming loop so it can be reasoned about (and
/// unit-tested) on its own.
enum Decoded {
    /// A domain fragment to run through the [`Inspector`].
    Inspect(Fragment),
    /// Nothing the proxy inspects (framing marker, unknown type, non-object,
    /// non-JSON): forward the raw frame unchanged.
    PassThrough,
    /// A recognized but malformed event (known `type`, missing/blank field) the
    /// policy can't see — fail closed by dropping it.
    MalformedDrop(AgUiError),
    /// Same, but the configured mode ends the run instead of dropping.
    MalformedTerminate(AgUiError),
}

/// Classify one event's `data` payload, applying the malformed-event `mode` to a
/// recognized-but-malformed event (one a content policy could otherwise be
/// bypassed by). Pure: no logging, metrics, or I/O.
fn decode(data: &str, mode: MalformedEventMode) -> Decoded {
    let Ok(value) = serde_json::from_str::<Value>(data) else {
        // Not JSON → not inspectable.
        return Decoded::PassThrough;
    };
    match to_fragment(&value) {
        Ok(Some(fragment)) => Decoded::Inspect(fragment),
        // Recognized but malformed: it must not slip past the policy, so fail
        // closed per the configured mode.
        Err(error) if error.is_malformed_known() => match mode {
            MalformedEventMode::Forward => Decoded::PassThrough,
            MalformedEventMode::Drop => Decoded::MalformedDrop(error),
            MalformedEventMode::Terminate => Decoded::MalformedTerminate(error),
        },
        // Not an object / no `type` / unknown type → nothing to inspect.
        Ok(None) | Err(_) => Decoded::PassThrough,
    }
}

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
/// How the stream is guarded comes in as one [`InspectionSettings`]: the
/// structural budgets, the malformed-event mode (a **recognized but malformed**
/// event — a known `type` with a missing/blank required field — cannot be
/// inspected yet belongs to the run, so forwarding it raw would bypass the
/// policy; the default fails closed and terminates), and the per-run response
/// budget (crossing it ends the run with a `RUN_ERROR` so a runaway agent
/// cannot flood the client).
pub fn inspect_stream(
    mut upstream: AgentResponseStream,
    inspector: Arc<Inspector>,
    context: InspectionContext,
    settings: InspectionSettings,
    metrics: Arc<dyn ProxyMetrics>,
) -> impl Stream<Item = Bytes> + Send {
    async_stream::stream! {
        let mut decoder = SseDecoder::new();
        let mut run = Run::new(context.run, settings.budgets);
        let mut pending: Vec<String> = Vec::new();
        let mut seen_events: usize = 0;
        let mut seen_bytes: usize = 0;

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
                // Count what the upstream sent and fail closed if the run's
                // response budget is crossed (DoS guard against unbounded output).
                seen_events += 1;
                seen_bytes += event.raw.len();
                if let Some(reason) = settings.response_budget.exceeded(seen_events, seen_bytes) {
                    warn!(run = %context.run, reason, "terminating run: response budget exceeded");
                    metrics.record_inspected(InspectionOutcome::Terminate);
                    pending.clear();
                    yield Bytes::from(run_error(reason));
                    return;
                }

                let fragment = match decode(&event.data, settings.malformed_mode) {
                    Decoded::Inspect(fragment) => fragment,
                    Decoded::PassThrough => {
                        // not inspectable: forward (preserving order during a hold)
                        if pending.is_empty() {
                            yield Bytes::from(event.raw);
                        } else {
                            pending.push(event.raw);
                        }
                        continue;
                    }
                    Decoded::MalformedDrop(error) => {
                        warn!(run = %context.run, %error, "dropping a malformed known event");
                        metrics.record_inspected(InspectionOutcome::Deny);
                        continue;
                    }
                    Decoded::MalformedTerminate(error) => {
                        warn!(run = %context.run, %error, "terminating run on a malformed known event");
                        metrics.record_inspected(InspectionOutcome::Terminate);
                        pending.clear();
                        yield Bytes::from(run_error("malformed protocol event"));
                        return;
                    }
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
    use crate::application::inspection::ResponseBudget;
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

    #[test]
    fn decode_classifies_an_inspectable_event() {
        let decoded = decode("{\"type\":\"RUN_STARTED\"}", MalformedEventMode::Terminate);
        assert!(matches!(decoded, Decoded::Inspect(_)));
    }

    #[test]
    fn decode_passes_through_unknown_non_object_and_non_json() {
        assert!(matches!(
            decode(
                "{\"type\":\"SOME_FUTURE_EVENT\"}",
                MalformedEventMode::Terminate
            ),
            Decoded::PassThrough
        ));
        assert!(matches!(
            decode("[1,2,3]", MalformedEventMode::Terminate),
            Decoded::PassThrough
        ));
        assert!(matches!(
            decode("not json", MalformedEventMode::Terminate),
            Decoded::PassThrough
        ));
    }

    #[test]
    fn decode_applies_the_malformed_mode_to_a_recognized_but_malformed_event() {
        // TOOL_CALL_START with no toolCallName — recognized but malformed.
        let data = "{\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\"}";
        assert!(matches!(
            decode(data, MalformedEventMode::Forward),
            Decoded::PassThrough
        ));
        assert!(matches!(
            decode(data, MalformedEventMode::Drop),
            Decoded::MalformedDrop(_)
        ));
        assert!(matches!(
            decode(data, MalformedEventMode::Terminate),
            Decoded::MalformedTerminate(_)
        ));
    }

    /// Run `inspect_stream` with the default budgets/context, a fail-closed
    /// malformed mode, and an unlimited response budget — the shape every test
    /// below shares.
    fn inspect(
        upstream: AgentResponseStream,
        inspector: Arc<Inspector>,
        metrics: Arc<dyn ProxyMetrics>,
    ) -> impl Stream<Item = Bytes> + Send {
        inspect_stream(
            upstream,
            inspector,
            context(),
            InspectionSettings::default(),
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

    /// In `Drop` mode the malformed frame vanishes but the run continues:
    /// later well-formed events still reach the client.
    #[tokio::test]
    async fn a_malformed_known_event_is_dropped_in_drop_mode_and_the_stream_continues() {
        let stream = inspect_stream(
            upstream(&[
                "data: {\"type\":\"RUN_STARTED\"}\n\n",
                // TOOL_CALL_START with no toolCallName — recognized but malformed.
                "data: {\"type\":\"TOOL_CALL_START\",\"toolCallId\":\"c1\"}\n\n",
                "data: {\"type\":\"RUN_FINISHED\"}\n\n",
            ]),
            inspector(Arc::new(AllowAllPolicy)),
            context(),
            InspectionSettings {
                malformed_mode: MalformedEventMode::Drop,
                ..InspectionSettings::default()
            },
            metrics(),
        );

        let out = collect(stream).await;
        assert!(
            !out.contains("TOOL_CALL_START"),
            "the malformed frame is dropped: {out}"
        );
        assert!(
            !out.contains("RUN_ERROR"),
            "dropping does not terminate the run: {out}"
        );
        assert!(
            out.contains("RUN_STARTED") && out.contains("RUN_FINISHED"),
            "events around the dropped frame still stream: {out}"
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
            InspectionSettings {
                malformed_mode: MalformedEventMode::Forward,
                ..InspectionSettings::default()
            },
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

    /// A run that streams more events than its budget allows is terminated with
    /// a `RUN_ERROR` instead of flooding the client.
    #[tokio::test]
    async fn a_run_over_its_event_budget_is_terminated() {
        let stream = inspect_stream(
            upstream(&[
                "data: {\"type\":\"RUN_STARTED\"}\n\n",
                "data: {\"type\":\"TEXT_MESSAGE_CONTENT\",\"messageId\":\"m1\",\"delta\":\"a\"}\n\n",
                "data: {\"type\":\"TEXT_MESSAGE_CONTENT\",\"messageId\":\"m1\",\"delta\":\"b\"}\n\n",
                "data: {\"type\":\"RUN_FINISHED\"}\n\n",
            ]),
            inspector(Arc::new(AllowAllPolicy)),
            context(),
            InspectionSettings {
                response_budget: ResponseBudget {
                    max_events: 2,
                    max_bytes: 0,
                },
                ..InspectionSettings::default()
            },
            metrics(),
        );

        let out = collect(stream).await;
        assert!(
            out.contains("RUN_ERROR"),
            "budget over-run terminates: {out}"
        );
        assert!(
            !out.contains("RUN_FINISHED"),
            "the stream ends before the run completes: {out}"
        );
    }
}
