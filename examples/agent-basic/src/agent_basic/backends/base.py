"""The agent backend port (a structural ``Protocol``)."""

from __future__ import annotations

from collections.abc import AsyncIterator
from typing import Any, Protocol, runtime_checkable

from agent_basic.run_input import RunAgentInput


@runtime_checkable
class AgentBackend(Protocol):
    """Turns a ``RunAgentInput`` into a stream of AG-UI event dicts.

    Implementations yield plain event dicts (built with ``agent_basic.ag_ui``);
    the HTTP layer is responsible for SSE-framing them. Yielding dicts (not
    bytes) keeps backends transport-agnostic and easy to unit-test.
    """

    async def run(self, request: RunAgentInput) -> AsyncIterator[dict[str, Any]]:
        """Yield AG-UI events for one agent run, ending with ``RUN_FINISHED``."""
        ...
