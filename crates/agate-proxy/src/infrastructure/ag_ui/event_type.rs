//! AG-UI event-type discriminators (the `type` field), as a single source of
//! truth. Only the security-relevant subset the proxy inspects is named here;
//! every other (and future) type is treated as pass-through by the mapper.

pub const RUN_STARTED: &str = "RUN_STARTED";
pub const RUN_FINISHED: &str = "RUN_FINISHED";
pub const RUN_ERROR: &str = "RUN_ERROR";
pub const STEP_STARTED: &str = "STEP_STARTED";
pub const STEP_FINISHED: &str = "STEP_FINISHED";
pub const TEXT_MESSAGE_CONTENT: &str = "TEXT_MESSAGE_CONTENT";
/// The self-contained streaming form of assistant text (`messageId` + `delta`
/// in one frame, no `START`/`END` envelope). Real AG-UI agents commonly emit
/// this instead of `TEXT_MESSAGE_CONTENT`; the proxy inspects both identically.
pub const TEXT_MESSAGE_CHUNK: &str = "TEXT_MESSAGE_CHUNK";
pub const TOOL_CALL_START: &str = "TOOL_CALL_START";
pub const TOOL_CALL_ARGS: &str = "TOOL_CALL_ARGS";
pub const TOOL_CALL_END: &str = "TOOL_CALL_END";
pub const TOOL_CALL_RESULT: &str = "TOOL_CALL_RESULT";
pub const STATE_SNAPSHOT: &str = "STATE_SNAPSHOT";
pub const STATE_DELTA: &str = "STATE_DELTA";
pub const RAW: &str = "RAW";
pub const CUSTOM: &str = "CUSTOM";
pub const REASONING_ENCRYPTED_VALUE: &str = "REASONING_ENCRYPTED_VALUE";
