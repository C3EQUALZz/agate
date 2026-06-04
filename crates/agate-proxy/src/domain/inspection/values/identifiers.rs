use uuid::Uuid;

use crate::domain::common::values::ValueObject;

/// One agent execution (an AG-UI `runId`): the unit a verdict applies within.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RunId(pub Uuid);

impl ValueObject for RunId {}

/// A conversation spanning multiple runs (an AG-UI `threadId`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(pub Uuid);

impl ValueObject for SessionId {}

/// Correlates the start, streamed arguments, and result of one tool call.
///
/// Carries the protocol's own opaque id (a string, not a UUID) so the proxy can
/// match `TOOL_CALL_START` / `TOOL_CALL_ARGS` / `TOOL_CALL_END` frames.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ToolCallId(pub String);

impl ValueObject for ToolCallId {}

/// Correlates the streamed fragments of one assistant text message.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MessageId(pub String);

impl ValueObject for MessageId {}
