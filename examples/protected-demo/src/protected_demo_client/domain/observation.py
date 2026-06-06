"""Interpret the AG-UI events Agate emits, in terms of Agate's protections.

The client cannot see Agate's internal verdicts; it infers them from the
*observable* stream: a denied tool call never arrives (and the run ends in a
``RUN_ERROR``), and a redacted secret shows up as ``[REDACTED]`` in the text.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

# Markers that, if present in delta text, indicate Agate redacted a secret.
_REDACTION_MARKERS = ("[REDACTED]", "REDACTED")


@dataclass(slots=True)
class Observation:
    """A running tally of what Agate did, accumulated over the event stream."""

    saw_search: bool = False
    saw_delete: bool = False
    saw_redaction: bool = False
    saw_run_error: bool = False
    tool_calls: list[str] = field(default_factory=list)

    def observe(self, event: dict[str, Any]) -> None:
        kind = event.get("type")
        if kind == "TOOL_CALL_START":
            name = str(event.get("toolCallName", "?"))
            self.tool_calls.append(name)
            self.saw_search = self.saw_search or name == "search_documents"
            self.saw_delete = self.saw_delete or name == "delete_file"
        elif kind == "TEXT_MESSAGE_CONTENT":
            delta = str(event.get("delta", ""))
            if any(marker in delta for marker in _REDACTION_MARKERS):
                self.saw_redaction = True
        elif kind == "RUN_ERROR":
            self.saw_run_error = True

    @property
    def allowed_tool_passed(self) -> bool:
        return self.saw_search

    @property
    def dangerous_tool_blocked(self) -> bool:
        # The dangerous call must NOT have reached us.
        return not self.saw_delete
