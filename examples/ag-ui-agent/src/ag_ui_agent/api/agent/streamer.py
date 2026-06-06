"""The AG-UI streaming port the chat route depends on.

The real ``autogen.beta`` agent backend implements this single Protocol, so the
chat route (and Agate in front of it) sees one uniform AG-UI SSE contract and
never imports ``autogen`` directly. Keeping a port here also leaves room for an
alternative backend without touching the route.
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
        r"""Yield ``text/event-stream`` frames (``data: {json}\n\n``).

        ``bytes`` or ``str`` -- Starlette's ``StreamingResponse`` accepts both;
        ``AGUIStream`` yields the frames AG2 produces.
        """
        ...
