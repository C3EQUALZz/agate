"""Container construction, branching on the configured backend.

* ``stub`` — a plain dishka container (``Scope.APP`` / ``Scope.REQUEST``) with
  the scripted streamer. No autogen, no API key.
* ``ag2`` — a ``dishka-ag2`` container scoped with ``AG2Scope``, plus
  ``AG2Provider`` and the AG2 streamer. Imported lazily so the stub path never
  needs the ``ag2`` extra.

The shared providers (settings, repositories, use cases) take their scope as a
constructor argument, so the *same* provider classes serve both scope families.
"""

from typing import Any

from dishka import AsyncContainer, Provider, Scope, make_async_container

from ag_ui_agent.config import AgentBackend
from ag_ui_agent.main.providers import (
    RepositoryProvider,
    SettingsProvider,
    StubAgentProvider,
    UseCaseProvider,
)


def stub_providers() -> tuple[Provider, ...]:
    # FastapiProvider lets dishka's FastAPI integration pass ``Request`` into
    # request-scoped factories and manage the request scope for ``@inject``.
    from dishka.integrations.fastapi import FastapiProvider

    return (
        SettingsProvider(app_scope=Scope.APP),
        RepositoryProvider(app_scope=Scope.APP),
        UseCaseProvider(request_scope=Scope.REQUEST),
        StubAgentProvider(),
        FastapiProvider(),
    )


def create_stub_container(context: dict[Any, Any] | None = None) -> AsyncContainer:
    return make_async_container(*stub_providers(), context=context)


def ag2_providers() -> tuple[Provider, ...]:
    # Imported here (not at module top) so the stub path does not require the
    # optional ``ag2`` extra to be installed.
    from dishka_ag2 import AG2Provider, AG2Scope

    from ag_ui_agent.main.providers.agent_ag2 import Ag2AgentProvider

    return (
        SettingsProvider(app_scope=AG2Scope.APP),
        RepositoryProvider(app_scope=AG2Scope.APP),
        UseCaseProvider(request_scope=AG2Scope.REQUEST),
        Ag2AgentProvider(),
        AG2Provider(),
    )


def create_ag2_container(context: dict[Any, Any] | None = None) -> AsyncContainer:
    from dishka_ag2 import AG2Scope

    return make_async_container(*ag2_providers(), context=context, scopes=AG2Scope)


def create_container(
    backend: AgentBackend,
    context: dict[Any, Any] | None = None,
) -> AsyncContainer:
    if backend is AgentBackend.AG2:
        return create_ag2_container(context=context)
    return create_stub_container(context=context)
