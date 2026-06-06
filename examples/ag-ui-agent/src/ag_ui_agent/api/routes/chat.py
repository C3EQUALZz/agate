"""The AG-UI run endpoint — what Agate forwards inspected runs to.

Mounted at ``POST /api/run`` (so a proxy points ``agent_endpoint`` here). The
route has essentially zero agent plumbing: it resolves the :class:`AgUiStreamer`
port from Dishka and streams its frames. The concrete streamer (the real
``autogen.beta`` agent) is wired entirely by DI.
"""

from typing import Annotated

from dishka.integrations.fastapi import FromDishka, inject
from fastapi import APIRouter, Body, Header
from fastapi.responses import StreamingResponse

from ag_ui_agent.api.agent import AgUiStreamer
from ag_ui_agent.api.agent.run_input import RunAgentInputModel

router = APIRouter(tags=["chat"])


@router.post("/run")
@inject
async def run_agent(
    run_input: Annotated[RunAgentInputModel, Body()],
    streamer: FromDishka[AgUiStreamer],
    accept: Annotated[str | None, Header()] = None,
) -> StreamingResponse:
    """Stream the agent's AG-UI run as ``text/event-stream`` frames."""
    return StreamingResponse(
        streamer.dispatch(run_input, accept=accept),
        media_type=accept or "text/event-stream",
    )
