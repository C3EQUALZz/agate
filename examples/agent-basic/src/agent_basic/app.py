"""The FastAPI application exposing the AG-UI endpoint Agate forwards to.

Agate's ``[proxy].agent_endpoint`` points at ``http://<this>/run``. The contract:

  client/Agate  --POST /run, RunAgentInput JSON-->  this app
  this app      --text/event-stream (AG-UI SSE)-->  client/Agate

Two backends, chosen by ``AGENT_BACKEND``:

- ``stub`` (default): a ``POST /run`` handler streams the scripted events.
- ``ag2``: AG2's ``AGUIStream`` ASGI app is mounted at ``/run`` (it owns its own
  SSE transport, so we mount rather than re-stream).

dishka is wired with ``setup_dishka``; the stub handler receives its backend via
``FromDishka`` injection — no globals, no manual container lookups.
"""

from __future__ import annotations

from collections.abc import AsyncIterator
from contextlib import asynccontextmanager

from dishka import AsyncContainer, make_async_container
from dishka.integrations.fastapi import FromDishka, inject, setup_dishka
from fastapi import FastAPI, Request
from fastapi.responses import PlainTextResponse, StreamingResponse

from agent_basic import ag_ui
from agent_basic.backends import AgentBackend
from agent_basic.backends.ag2_backend import Ag2Backend
from agent_basic.config import AgentConfig
from agent_basic.providers import AppProvider

# AG-UI responses are Server-Sent Events.
SSE_MEDIA_TYPE = "text/event-stream"


def create_app() -> FastAPI:
    """Build the FastAPI app, wiring dishka and selecting the backend route."""
    container: AsyncContainer = make_async_container(AppProvider())
    config = AgentConfig.from_env()

    @asynccontextmanager
    async def lifespan(_: FastAPI) -> AsyncIterator[None]:
        # Close the dishka container (and any app-scoped resources) on shutdown.
        try:
            yield
        finally:
            await container.close()

    app = FastAPI(title="agent-basic (AG-UI)", lifespan=lifespan)
    setup_dishka(container, app)

    @app.get("/healthz")
    async def healthz() -> PlainTextResponse:
        return PlainTextResponse("ok")

    if config.backend == "ag2":
        # AG2 owns the SSE transport; mount its ASGI app at /run.
        app.mount("/run", Ag2Backend(config).build_asgi())
    else:
        _register_stub_run(app)

    return app


def _register_stub_run(app: FastAPI) -> None:
    """Register the ``POST /run`` handler for the dict-yielding stub backend."""

    @app.post("/run")
    @inject
    async def run(request: Request, backend: FromDishka[AgentBackend]) -> StreamingResponse:
        body = await _read_json(request)
        run_input = _parse_input(body)

        async def event_stream() -> AsyncIterator[bytes]:
            async for event in backend.run(run_input):
                yield ag_ui.sse_frame(event)

        return StreamingResponse(event_stream(), media_type=SSE_MEDIA_TYPE)


async def _read_json(request: Request) -> dict:
    try:
        decoded = await request.json()
    except Exception:
        return {}
    return decoded if isinstance(decoded, dict) else {}


def _parse_input(body: dict):
    # Imported lazily to keep this module's import graph shallow.
    from agent_basic.run_input import RunAgentInput

    return RunAgentInput.parse(body)


# Importable target for ``uvicorn agent_basic.app:app``.
app = create_app()
