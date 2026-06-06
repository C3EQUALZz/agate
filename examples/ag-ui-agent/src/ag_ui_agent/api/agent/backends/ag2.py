"""Real-AG2 backend: an ``autogen.beta.Agent`` streamed over AG-UI.

Imported only when ``AGENT__BACKEND=ag2`` and the ``ag2`` extra is installed.
This is the exact ``dishka-ag2`` + ``AGUIStream`` wiring from the public
reference (https://github.com/vvlrff/ag2_ag-ui_example):

* the agent is an APP-level singleton built once, with ``DishkaAsyncMiddleware``
  attached so tool calls resolve ``FromDishka[...]`` from the container;
* ``AGUIStream(agent).dispatch(run_input, accept=accept)`` produces the AG-UI
  SSE byte stream.
"""

from collections.abc import AsyncIterator

from autogen.beta import Agent
from autogen.beta.ag_ui import AGUIStream, RunAgentInput
from autogen.beta.config import OpenAIConfig
from autogen.beta.middleware import Middleware
from dishka import AsyncContainer
from dishka_ag2 import DishkaAsyncMiddleware

from ag_ui_agent.api.agent.prompts import SYSTEM_PROMPT
from ag_ui_agent.api.agent.run_input import RunAgentInputModel
from ag_ui_agent.api.agent.tools import documents_toolkit, echo_status


def build_agent(config: OpenAIConfig, container: AsyncContainer) -> Agent:
    """Build the AG2 agent with the document toolkit and Dishka middleware."""
    return Agent(
        name="workspace_assistant",
        prompt=SYSTEM_PROMPT,
        config=config,
        tools=[echo_status, documents_toolkit()],
        middleware=[Middleware(DishkaAsyncMiddleware, container=container)],
    )


class Ag2AgUiStreamer:
    """Adapt :class:`AGUIStream` to the :class:`AgUiStreamer` port."""

    def __init__(self, agent: Agent) -> None:
        self._agent = agent

    def dispatch(
        self,
        run_input: RunAgentInputModel,
        accept: str | None = None,
    ) -> AsyncIterator[bytes | str]:
        # VERIFY: RunAgentInput is the autogen.beta.ag_ui pydantic model; it
        # accepts the camelCase AG-UI fields. We rebuild it from our neutral
        # model's wire form so the route never imports autogen. If the field set
        # drifts, pass the parsed body straight through instead.
        native = RunAgentInput.model_validate(run_input.model_dump(by_alias=True))
        # VERIFY: AGUIStream.dispatch(run_input, accept=accept) returns the AG-UI
        # SSE async iterator (the reference's exact call). Type ignored because
        # autogen ships no stubs for the default (stub-only) environment.
        return AGUIStream(self._agent).dispatch(native, accept=accept)  # type: ignore[no-any-return]
