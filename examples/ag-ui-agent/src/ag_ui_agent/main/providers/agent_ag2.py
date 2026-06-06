"""DI wiring for the real-AG2 backend (loaded only when ``backend=ag2``).

Kept in its own module so importing it — and thus ``autogen`` / ``dishka_ag2`` —
happens lazily, only when the ``ag2`` extra is installed and selected.

The agent is an APP-scoped singleton built by a factory that receives the live
``AsyncContainer``: that is how its ``DishkaAsyncMiddleware`` gets the container
reference it needs to open ``AG2Scope.REQUEST`` per tool call. Letting dishka
inject the container into the factory breaks the agent<->container cycle without
a second container or app-state juggling.
"""

import httpx
from autogen.beta import Agent
from autogen.beta.config import OpenAIConfig
from dishka import AsyncContainer, Provider, provide
from dishka_ag2 import AG2Scope

from ag_ui_agent.api.agent import AgUiStreamer
from ag_ui_agent.api.agent.backends.ag2 import Ag2AgUiStreamer, build_agent
from ag_ui_agent.config import Settings


class Ag2AgentProvider(Provider):
    """Provide the OpenAI config, the AG2 agent, and the AG-UI streamer."""

    @provide(scope=AG2Scope.APP)
    def provide_openai_config(self, settings: Settings) -> OpenAIConfig:
        kwargs: dict[str, object] = {
            "model": settings.openai_model,
            "api_key": settings.openai_api_key.get_secret_value(),
        }
        if settings.openai_proxy_url:
            # OpenAIConfig forwards http_client to the OpenAI SDK — the proxy
            # path the reference uses (entrypoint builds OpenAIConfig the same way).
            kwargs["http_client"] = httpx.AsyncClient(proxy=settings.openai_proxy_url)
        return OpenAIConfig(**kwargs)

    @provide(scope=AG2Scope.APP)
    def provide_agent(self, config: OpenAIConfig, container: AsyncContainer) -> Agent:
        return build_agent(config, container)

    @provide(scope=AG2Scope.REQUEST)
    def provide_streamer(self, agent: Agent) -> AgUiStreamer:
        return Ag2AgUiStreamer(agent=agent)
