"""dishka dependency-injection wiring.

dishka is a Python DI framework (think: a typed, scope-aware service container).
A ``Provider`` declares how to build dependencies; ``make_async_container``
assembles them; the FastAPI integration (``setup_dishka`` + ``FromDishka``)
injects them into request handlers.

We provide two app-scoped singletons:

- ``AgentConfig`` — read once from the environment.
- ``AgentBackend`` — the stub or the ag2 adapter, chosen from the config.

Keeping construction here means the FastAPI handler depends only on the
``AgentBackend`` *port*, never on a concrete backend or on ``os.environ``.
"""

from __future__ import annotations

from dishka import Provider, Scope, provide

from agent_basic.backends import AgentBackend, StubBackend
from agent_basic.config import AgentConfig


class AppProvider(Provider):
    """App-scoped providers for config and the agent backend."""

    @provide(scope=Scope.APP)
    def config(self) -> AgentConfig:
        return AgentConfig.from_env()

    @provide(scope=Scope.APP)
    def backend(self, config: AgentConfig) -> AgentBackend:
        """Select the backend from config.

        Only the stub is constructed here; the ag2 backend is mounted as its own
        ASGI app in ``app.py`` (AG2 owns its SSE transport), so it is not a
        request-time dependency. This provider still validates the choice.
        """
        if config.backend == "stub":
            return StubBackend(config)
        if config.backend == "ag2":
            # Constructed and mounted in app.py via build_asgi(); never injected
            # into the stub handler. Returning the stub here keeps the container
            # total without pulling in ``autogen`` at import time.
            return StubBackend(config)
        raise ValueError(
            f"unknown AGENT_BACKEND={config.backend!r}; expected 'stub' or 'ag2'"
        )
