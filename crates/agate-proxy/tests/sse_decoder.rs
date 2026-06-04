//! The SSE decoder: incremental, order-preserving framing with exact raw bytes.

use agate_proxy::infrastructure::sse::{SseDecoder, encode};

#[test]
fn decodes_a_single_event() {
    let mut decoder = SseDecoder::new();
    let events = decoder.push(b"data: {\"type\":\"RUN_STARTED\"}\n\n");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "{\"type\":\"RUN_STARTED\"}");
    assert_eq!(events[0].raw, "data: {\"type\":\"RUN_STARTED\"}\n\n");
}

#[test]
fn buffers_an_event_split_across_chunks() {
    let mut decoder = SseDecoder::new();

    assert!(decoder.push(b"data: {\"ty").is_empty());
    assert!(decoder.push(b"pe\":\"RUN_STARTED\"}").is_empty());
    let events = decoder.push(b"\n\n");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "{\"type\":\"RUN_STARTED\"}");
}

#[test]
fn decodes_multiple_events_in_one_chunk() {
    let mut decoder = SseDecoder::new();
    let events = decoder.push(b"data: a\n\ndata: b\n\ndata: c\n\n");

    let payloads: Vec<_> = events.iter().map(|event| event.data.as_str()).collect();
    assert_eq!(payloads, ["a", "b", "c"]);
}

#[test]
fn concatenates_multi_line_data() {
    let mut decoder = SseDecoder::new();
    let events = decoder.push(b"data: line one\ndata: line two\n\n");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "line one\nline two");
}

#[test]
fn handles_crlf_terminators_and_event_field() {
    let mut decoder = SseDecoder::new();
    let events = decoder.push(b"event: message\r\ndata: hello\r\n\r\n");

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.as_deref(), Some("message"));
    assert_eq!(events[0].data, "hello");
}

#[test]
fn ignores_comment_keepalives() {
    let mut decoder = SseDecoder::new();
    // A comment-only block carries no data and is not emitted.
    assert!(decoder.push(b": keep-alive\n\n").is_empty());
    let events = decoder.push(b"data: real\n\n");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data, "real");
}

#[test]
fn preserves_exact_raw_bytes_for_forwarding() {
    let mut decoder = SseDecoder::new();
    let frame = "data: {\"k\": 1}\n\n";
    let events = decoder.push(frame.as_bytes());
    assert_eq!(events[0].raw, frame);
}

#[test]
fn encode_produces_an_sse_frame() {
    assert_eq!(
        encode("{\"type\":\"RUN_ERROR\"}"),
        "data: {\"type\":\"RUN_ERROR\"}\n\n"
    );
}
