"""The AG-UI streaming port the chat route depends on.

Both backends — the offline stub and the real ``autogen.beta`` agent — implement
this single Protocol, so the chat route (and Agate in front of it) sees one
uniform AG-UI SSE contract regardless of which backend is wired in. Selecting a
backend is a one-line DI change in ``main/providers/agent.py``; no route changes.
"""

from collections.abc import AsyncIterator
from typing import Protocol

from ag_ui_agent.api.agent.run_input import RunAgentInputModel


class AgUiStreamer(Protocol):
    """Turns a parsed AG-UI ``RunAgentInput`` into an AG-UI SSE byte stream."""

    def dispatch(
        self,
        run_input: RunAgentInputModel,
        accept: str | None = None,
    ) -> AsyncIterator[bytes | str]:
        """Yield ``text/event-stream`` frames (``data: {json}\\n\\n``).

        ``bytes`` or ``str`` — Starlette's ``StreamingResponse`` accepts both.
        The stub yields ``bytes``; ``AGUIStream`` yields the frames AG2 produces.
        """
        ...
