"""Composition root: build the container + agent and wire the FastAPI app.

There is one backend (the real ``autogen.beta`` agent), so ``create_app`` builds
one ``AG2Scope`` container. The agent is an ``AG2Scope.APP`` singleton built by a
factory that receives the container by injection (for its
``DishkaAsyncMiddleware``); a small ASGI middleware opens ``AG2Scope.REQUEST``
per HTTP request so the REST routes' ``@inject`` keeps working.
"""

from collections.abc import AsyncIterator
from contextlib import asynccontextmanager

import uvicorn
from dishka import AsyncContainer
from fastapi import FastAPI

from ag_ui_agent.api.middlewares import RequestIDMiddleware
from ag_ui_agent.api.routes import routers as api_routers
from ag_ui_agent.config import Settings
from ag_ui_agent.logging_config import configure_logging
from ag_ui_agent.main.di import create_container
from ag_ui_agent.main.middleware import AG2ContainerMiddleware

_HOST = "0.0.0.0"  # noqa: S104  # demo container binds all interfaces on purpose.
_PORT = 8000
_DESCRIPTION = "Clean-layered AG2.beta + AG-UI + Dishka agent (the upstream Agate protects)."


def build_app(settings: Settings, container: AsyncContainer) -> FastAPI:
    """Wire a FastAPI app around an already-built container.

    Kept separate from :func:`create_app` so tests can supply a container that
    swaps the live agent for a fake ``AgUiStreamer`` (no autogen, no API call).
    """
    configure_logging(level=settings.log_level, json_output=settings.log_json)

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[None]:
        yield
        await container.close()

    app = FastAPI(
        title=settings.app_name,
        description=_DESCRIPTION,
        version="0.1.0",
        lifespan=lifespan,
    )
    app.add_middleware(AG2ContainerMiddleware, container=container)
    app.state.dishka_container = container

    app.add_middleware(RequestIDMiddleware)
    app.router.prefix = "/api"
    for router in api_routers:
        app.include_router(router)
    return app


def create_app(settings: Settings | None = None) -> FastAPI:
    """Build the production app: the real container wired into FastAPI."""
    # VERIFY: Settings() reads required fields (openai_api_key) from the env, so
    # mypy sees a missing-arg unless the pydantic-settings plugin is active. If
    # warn_unused_ignores flags this ignore as unneeded, drop it.
    settings = settings or Settings()  # type: ignore[call-arg]
    container = create_container(context={Settings: settings})
    return build_app(settings, container)


def run() -> None:
    """Console-script entrypoint: serve the app with uvicorn.

    Uses uvicorn's *factory* mode (``create_app`` is called by uvicorn) so the
    app -- and thus the real container and ``Settings`` (which require an API
    key) -- is built only when the server actually starts, not at import time.
    """
    uvicorn.run(
        "ag_ui_agent.main.entrypoint:create_app",
        factory=True,
        host=_HOST,
        port=_PORT,
        log_config=None,
    )
