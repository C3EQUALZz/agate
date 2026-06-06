"""The AG-UI wire protocol: event constructors and Server-Sent-Events framing.

AG-UI is an HTTP ``POST`` of a ``RunAgentInput`` JSON body (client -> agent)
followed by a stream of events as ``text/event-stream`` (SSE) in the response
(agent -> client). Each event is a JSON object with a ``type`` discriminator and
**camelCase** fields.

The field names below are chosen to match exactly what **Agate's** proxy parses
(see ``crates/agate-proxy/src/infrastructure/ag_ui/mapper.rs``): ``messageId`` /
``delta`` for text, ``toolCallId`` / ``toolCallName`` for tool calls, ``content``
for tool results. Keeping them exact is what lets Agate inspect, allow, deny and
redact our events.

This module deliberately hand-rolls the events instead of depending on the
``ag-ui-protocol`` Python SDK, so the stub backend runs with zero third-party
deps and the emitted bytes are obvious and auditable. AG2's own ``AGUIStream``
emits the same protocol; see ``backends/ag2_backend.py``.
"""

from __future__ import annotations

import json
from collections.abc import Iterable, Mapping
from typing import Any

# --- Event type discriminators (the AG-UI ``type`` field). --------------------
# This is the security-relevant subset Agate names explicitly; every other type
# is treated as pass-through. Mirrors agate-proxy's ``event_type.rs``.
RUN_STARTED = "RUN_STARTED"
RUN_FINISHED = "RUN_FINISHED"
RUN_ERROR = "RUN_ERROR"
TEXT_MESSAGE_START = "TEXT_MESSAGE_START"
TEXT_MESSAGE_CONTENT = "TEXT_MESSAGE_CONTENT"
TEXT_MESSAGE_END = "TEXT_MESSAGE_END"
TOOL_CALL_START = "TOOL_CALL_START"
TOOL_CALL_ARGS = "TOOL_CALL_ARGS"
TOOL_CALL_END = "TOOL_CALL_END"
TOOL_CALL_RESULT = "TOOL_CALL_RESULT"


def sse_frame(event: Mapping[str, Any]) -> bytes:
    """Encode one AG-UI event as a single SSE ``data:`` frame.

    The frame is ``data: <json>\\n\\n`` — the exact shape Agate's incremental SSE
    codec decodes (one JSON object per event block, ``\\n\\n``-terminated).
    ``separators`` keeps the JSON compact so a frame is one line.
    """
    payload = json.dumps(event, separators=(",", ":"))
    return f"data: {payload}\n\n".encode()


# --- Event constructors. ------------------------------------------------------
# Each returns a plain dict; pass it through ``sse_frame`` to put it on the wire.


def run_started(thread_id: str, run_id: str) -> dict[str, Any]:
    return {"type": RUN_STARTED, "threadId": thread_id, "runId": run_id}


def run_finished(thread_id: str, run_id: str) -> dict[str, Any]:
    return {"type": RUN_FINISHED, "threadId": thread_id, "runId": run_id}


def run_error(message: str) -> dict[str, Any]:
    return {"type": RUN_ERROR, "message": message}


def text_message_start(message_id: str, role: str = "assistant") -> dict[str, Any]:
    return {"type": TEXT_MESSAGE_START, "messageId": message_id, "role": role}


def text_message_content(message_id: str, delta: str) -> dict[str, Any]:
    """A chunk of streamed assistant text. Agate inspects ``delta`` for secrets."""
    return {"type": TEXT_MESSAGE_CONTENT, "messageId": message_id, "delta": delta}


def text_message_end(message_id: str) -> dict[str, Any]:
    return {"type": TEXT_MESSAGE_END, "messageId": message_id}


def tool_call_start(tool_call_id: str, tool_call_name: str) -> dict[str, Any]:
    """Begin a tool call. Agate's allow/deny verdict keys off ``toolCallName``."""
    return {
        "type": TOOL_CALL_START,
        "toolCallId": tool_call_id,
        "toolCallName": tool_call_name,
    }


def tool_call_args(tool_call_id: str, delta: str) -> dict[str, Any]:
    """A fragment of the tool-call arguments (concatenated JSON string).

    Agate buffers every ``TOOL_CALL_ARGS`` between START and END so its verdict
    sees the *complete* arguments, not a single fragment.
    """
    return {"type": TOOL_CALL_ARGS, "toolCallId": tool_call_id, "delta": delta}


def tool_call_end(tool_call_id: str) -> dict[str, Any]:
    return {"type": TOOL_CALL_END, "toolCallId": tool_call_id}


def tool_call_result(tool_call_id: str, content: str) -> dict[str, Any]:
    return {"type": TOOL_CALL_RESULT, "toolCallId": tool_call_id, "content": content}


def stream_text(message_id: str, text: str, *, chunk: int = 16) -> Iterable[dict[str, Any]]:
    """Yield a START / CONTENT* / END sequence for a piece of assistant text.

    Splitting into chunks mimics token streaming and gives Agate several
    ``TEXT_MESSAGE_CONTENT`` frames to inspect.
    """
    yield text_message_start(message_id)
    for start in range(0, len(text), chunk):
        yield text_message_content(message_id, text[start : start + chunk])
    yield text_message_end(message_id)


def call_tool(
    tool_call_id: str, name: str, arguments: Mapping[str, Any]
) -> Iterable[dict[str, Any]]:
    """Yield a full START / ARGS / END tool-call sequence.

    The arguments are sent as a single ``TOOL_CALL_ARGS`` fragment here; a real
    agent may stream several. Agate buffers them either way.
    """
    yield tool_call_start(tool_call_id, name)
    yield tool_call_args(tool_call_id, json.dumps(arguments, separators=(",", ":")))
    yield tool_call_end(tool_call_id)
