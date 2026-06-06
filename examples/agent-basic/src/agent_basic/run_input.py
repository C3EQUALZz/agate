"""Parsing of the AG-UI ``RunAgentInput`` request body (client -> agent).

AG-UI sends a ``POST`` with a JSON ``RunAgentInput``. The fields we care about:

- ``threadId`` / ``runId`` — correlation ids echoed back in lifecycle events.
- ``messages`` — the conversation so far; the last ``user`` message is the
  prompt.

Everything else (``state``, ``tools``, ``context``, ``forwardedProps``) is
untyped ``any`` in the protocol and ignored here. (Agate is the component that
size-bounds and screens those untrusted fields.)
"""

from __future__ import annotations

import uuid
from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import Any


@dataclass(frozen=True, slots=True)
class RunAgentInput:
    thread_id: str
    run_id: str
    messages: list[dict[str, Any]] = field(default_factory=list)

    @classmethod
    def parse(cls, body: Mapping[str, Any]) -> RunAgentInput:
        """Parse a decoded JSON body, supplying ids if the client omitted them."""
        messages = body.get("messages") or []
        if not isinstance(messages, list):
            messages = []
        return cls(
            thread_id=str(body.get("threadId") or uuid.uuid4()),
            run_id=str(body.get("runId") or uuid.uuid4()),
            messages=messages,
        )

    @property
    def last_user_message(self) -> str:
        """The most recent user message text, or empty string if there is none."""
        for message in reversed(self.messages):
            if message.get("role") == "user":
                content = message.get("content", "")
                return content if isinstance(content, str) else str(content)
        return ""
