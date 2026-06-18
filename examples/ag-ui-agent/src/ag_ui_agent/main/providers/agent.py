"""DI wiring for the AG2 agent and the AG-UI streamer.

The agent is an ``AG2Scope.APP`` singleton built by a factory that receives the
live ``AsyncContainer``: that is how its ``DishkaAsyncMiddleware`` gets the
container reference it needs to open ``AG2Scope.REQUEST`` per tool call. Letting
dishka inject the container into the factory breaks the agent<->container cycle
without a second container or app-state juggling. The toolkit comes from
``ToolkitProvider``, so the agent never hand-builds its tools.
"""

import httpx
from autogen.beta import Agent, Toolkit
from autogen.beta.config import OpenAIConfig
from dishka import AsyncContainer, Provider, provide
from dishka_ag2 import AG2Scope

from ag_ui_agent.api.agent import AgUiStreamer
from ag_ui_agent.api.agent.backends import Ag2AgUiStreamer, build_agent
from ag_ui_agent.config import Settings


class AgentProvider(Provider):
    """Provide the OpenAI config, the AG2 agent, and the AG-UI streamer."""

    @provide(scope=AG2Scope.APP)
    def provide_openai_config(self, settings: Settings) -> OpenAIConfig:
        """Build the OpenAI config (optionally through an HTTP proxy).

        ``openai_base_url`` retargets any OpenAI-compatible provider (e.g.
        Mistral); ``None`` leaves the OpenAI default.
        """
        if settings.openai_proxy_url is None:
            return OpenAIConfig(
                model=settings.openai_model,
                api_key=settings.openai_api_key.get_secret_value(),
                base_url=settings.openai_base_url,
            )
        # OpenAIConfig forwards http_client to the OpenAI SDK -- the proxy path
        # the reference uses (entrypoint builds OpenAIConfig the same way).
        return OpenAIConfig(
            model=settings.openai_model,
            api_key=settings.openai_api_key.get_secret_value(),
            base_url=settings.openai_base_url,
            http_client=httpx.AsyncClient(proxy=settings.openai_proxy_url),
        )

    @provide(scope=AG2Scope.APP)
    def provide_agent(
        self,
        config: OpenAIConfig,
        toolkit: Toolkit,
        container: AsyncContainer,
    ) -> Agent:
        """Build the AG2 agent singleton with the injected toolkit + container."""
        return build_agent(config, toolkit, container)

    @provide(scope=AG2Scope.REQUEST)
    def provide_streamer(self, agent: Agent) -> AgUiStreamer:
        """Adapt the AG2 agent to the ``AgUiStreamer`` port."""
        return Ag2AgUiStreamer(agent=agent)
