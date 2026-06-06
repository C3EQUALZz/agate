"""A real AutoGen 2 (``ag2``) agent bridged to AG-UI.

AG2 ships an AG-UI bridge: ``autogen.ag_ui.AGUIStream`` wraps a
``ConversableAgent`` and translates its text, tool calls and lifecycle into the
AG-UI SSE event stream. ``AGUIStream.build_asgi()`` returns a mountable ASGI app.

We keep this backend in an **optional dependency group** (``--extra ag2``) so the
default stub mode stays key-free and dependency-light. Importing ``autogen`` is
deferred to call time so the package imports cleanly without it installed.

VERIFY (fast-moving / beta API — confirm after ``uv sync --extra ag2``):
  * Install:        ``ag2[openai,ag-ui]`` ships ``autogen`` + the AG-UI extra.
                    There is no separate ``ag2-beta`` distribution; the beta
                    framework lives under ``autogen.beta`` in the ``ag2`` package.
  * Import paths:   ``from autogen import ConversableAgent, LLMConfig`` and
                    ``from autogen.ag_ui import AGUIStream``.
  * Streaming:      ``LLMConfig(... , stream=True)`` so text streams as
                    ``TEXT_MESSAGE_CONTENT`` chunks.
  * ASGI:           ``AGUIStream(agent).build_asgi()`` -> ASGI app exposing the
                    AG-UI ``POST`` endpoint (Agate forwards to it as ``/run``).
                    If ``build_asgi`` is renamed/changed, adjust ``build_asgi``
                    below — that is the single integration seam.

DI note: AG2's *own* DI integration (``dishka-ag2``) targets the ``autogen.beta``
agent loop via middleware (``DishkaAsyncMiddleware``) and injects ``FromDishka``
deps into tools. ``AGUIStream`` here wraps a classic ``ConversableAgent`` and
does not expose those middleware hooks, so we use **plain dishka** to construct
the agent's collaborators (config, model) and document ``dishka-ag2`` rather than
forcing it onto a code path it does not yet cover. See README "Dependency
injection".
"""

from __future__ import annotations

from collections.abc import AsyncIterator
from typing import TYPE_CHECKING, Any

from agent_basic.config import AgentConfig
from agent_basic.run_input import RunAgentInput

if TYPE_CHECKING:  # pragma: no cover - typing only
    from starlette.types import ASGIApp


def get_weather(location: str) -> str:
    """A trivial, side-effect-free tool the agent may call.

    Real agents call out to APIs here. Open-Meteo etc. need no key; we return a
    canned string so the example stays offline-friendly.
    """
    return f"It is 21 C and sunny in {location}."


class Ag2Backend:
    """Builds a ``ConversableAgent`` and exposes it as an AG-UI ASGI app."""

    def __init__(self, config: AgentConfig) -> None:
        if not config.openai_api_key:
            raise RuntimeError(
                "AGENT_BACKEND=ag2 requires OPENAI_API_KEY. "
                "Use the default stub backend to run without an API key."
            )
        self._config = config

    def build_asgi(self) -> ASGIApp:
        """Construct the agent and return a mountable AG-UI ASGI app.

        Imports are local so the package works without ``ag2`` installed.
        """
        from autogen import ConversableAgent, LLMConfig  # type: ignore[import-not-found]
        from autogen.ag_ui import AGUIStream  # type: ignore[import-not-found]

        llm_config = LLMConfig(
            api_type="openai",
            model=self._config.model,
            api_key=self._config.openai_api_key,
            # Stream tokens so Agate receives TEXT_MESSAGE_CONTENT chunks to
            # inspect, instead of one final blob.
            stream=True,
        )

        agent = ConversableAgent(
            name="basic_agent",
            system_message=(
                "You are a concise assistant. Use the get_weather tool when asked "
                "about weather. Keep replies short."
            ),
            llm_config=llm_config,
            functions=[get_weather],  # VERIFY: tool-registration kwarg name.
        )

        # AGUIStream turns the agent into an AG-UI SSE endpoint. build_asgi()
        # yields the ASGI app we mount at /run in app.py.
        return AGUIStream(agent).build_asgi()

    async def run(self, request: RunAgentInput) -> AsyncIterator[dict[str, Any]]:
        # AG2 drives the SSE stream itself through the mounted ASGI app, so the
        # dict-yielding port is not used for this backend. app.py mounts
        # build_asgi() instead of routing through here.
        raise NotImplementedError(
            "Ag2Backend serves AG-UI via build_asgi(); it is mounted directly."
        )
