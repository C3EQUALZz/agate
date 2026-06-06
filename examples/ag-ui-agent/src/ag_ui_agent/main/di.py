"""Composition root: build the single ``dishka-ag2`` container.

There is exactly one backend (the real ``autogen.beta`` agent), so there is
exactly one container, scoped with ``dishka_ag2.AG2Scope``. ``AG2Scope.APP``
holds singletons (settings, repository, the agent itself); ``AG2Scope.REQUEST``
is opened on every HTTP request *and* on every agent tool call, which is how
``DishkaAsyncMiddleware`` resolves the tools' ``FromDishka[...]`` collaborators.

Providers are registered per concern, mirroring the reference
(https://github.com/vvlrff/ag2_ag-ui_example).
"""

from typing import Any

from dishka import AsyncContainer, make_async_container
from dishka_ag2 import AG2Provider, AG2Scope

from ag_ui_agent.main.providers import (
    AgentProvider,
    RepositoryProvider,
    SettingsProvider,
    ToolkitProvider,
    UseCaseProvider,
)


def create_container(context: dict[Any, Any] | None = None) -> AsyncContainer:
    """Build the application container with every provider registered."""
    return make_async_container(
        SettingsProvider(),
        RepositoryProvider(),
        UseCaseProvider(),
        ToolkitProvider(),
        AgentProvider(),
        AG2Provider(),
        context=context,
        scopes=AG2Scope,
    )
