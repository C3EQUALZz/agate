"""Composition root: build the container + agent and wire the FastAPI app.

``create_app`` branches on ``Settings.backend``:

* **stub** — a plain dishka container plus dishka's stock FastAPI integration
  (``setup_dishka``), which manages the request scope and ``@inject``.
* **ag2** — a ``dishka-ag2`` (``AG2Scope``) container; the real
  ``autogen.beta.Agent`` is an APP-scoped factory that receives the container by
  injection (for its ``DishkaAsyncMiddleware``), and a small ASGI middleware
  opens ``AG2Scope.REQUEST`` per HTTP request.

In both cases the chat route resolves the same :class:`AgUiStreamer` port.
"""

from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from typing import Any

from fastapi import FastAPI

from ag_ui_agent.api.middlewares import RequestIDMiddleware
from ag_ui_agent.api.routes import routers as api_routers
from ag_ui_agent.config import AgentBackend, Settings
from ag_ui_agent.logging_config import configure_logging
from ag_ui_agent.main.di import create_container


def create_app(settings: Settings | None = None) -> FastAPI:
    settings = settings or Settings()
    configure_logging(level=settings.log_level, json_output=settings.log_json)

    if settings.backend is AgentBackend.AG2:
        app = _create_ag2_app(settings)
    else:
        app = _create_stub_app(settings)

    app.add_middleware(RequestIDMiddleware)
    app.router.prefix = "/api"
    for router in api_routers:
        app.include_router(router)
    return app


def _base_app(settings: Settings, lifespan: Any) -> FastAPI:
    return FastAPI(
        title=settings.app_name,
        description="Clean-layered AG2.beta + AG-UI + Dishka agent (the upstream Agate protects).",
        version="0.1.0",
        lifespan=lifespan,
    )


def _create_stub_app(settings: Settings) -> FastAPI:
    from dishka.integrations.fastapi import setup_dishka

    from ag_ui_agent.main.di import create_stub_container

    container = create_stub_container(context={Settings: settings})

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[None]:
        yield
        await container.close()

    app = _base_app(settings, lifespan)
    setup_dishka(container=container, app=app)
    return app


def _create_ag2_app(settings: Settings) -> FastAPI:
    from ag_ui_agent.main.middleware import AG2ContainerMiddleware

    # One AG2Scope container. The agent (and its DishkaAsyncMiddleware) is an
    # APP-scoped factory that receives this container by injection, so there is
    # no agent<->container bootstrap cycle to untangle here.
    container = create_container(AgentBackend.AG2, context={Settings: settings})

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[None]:
        yield
        await container.close()

    app = _base_app(settings, lifespan)
    app.add_middleware(AG2ContainerMiddleware, container=container)
    app.state.dishka_container = container
    return app


def run() -> None:
    """Console-script entrypoint: serve the app with uvicorn."""
    import uvicorn

    uvicorn.run(
        "ag_ui_agent.main.entrypoint:app",
        host="0.0.0.0",  # demo container binds all interfaces.
        port=8000,
        log_config=None,
    )


app = create_app()
