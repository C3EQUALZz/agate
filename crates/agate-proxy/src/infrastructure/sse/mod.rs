//! Server-Sent Events transport: an incremental, order-preserving decoder for
//! the agent's response stream, plus an encoder for forwarding.
//!
//! AG-UI rides on SSE (`data: {json}\n\n`). The decoder keeps each event's raw
//! bytes so an allowed event is forwarded byte-for-byte, while a transformed one
//! is re-encoded with [`encode`].

pub mod decoder;
pub mod event;

pub use decoder::SseDecoder;
pub use event::SseEvent;

/// Encode a data payload as one SSE event (`data: {payload}\n\n`), matching
/// AG-UI's own encoder.
#[must_use]
pub fn encode(data: &str) -> String {
    format!("data: {data}\n\n")
}
