from collections.abc import Iterator

import pytest
from dishka import AsyncContainer, Provider, make_async_container, provide
from dishka_ag2 import AG2Scope
from fastapi import FastAPI
from fastapi.testclient import TestClient
from pydantic import SecretStr

from ag_ui_agent.api.agent import AgUiStreamer
from ag_ui_agent.config import Settings
from ag_ui_agent.main.entrypoint import build_app
from ag_ui_agent.main.providers import (
    RepositoryProvider,
    SettingsProvider,
    UseCaseProvider,
)
from ag_ui_agent.usecases import SearchDocumentsUseCase
from tests.fakes.streamer import FakeAgUiStreamer


class _FakeStreamerProvider(Provider):
    """Swap the live AG2 streamer for the scripted fake (no autogen, no key).

    Mirrors the production ``AgentProvider``'s ``AgUiStreamer`` binding, but
    builds a ``FakeAgUiStreamer`` from the real search use case, so the route,
    Dishka resolution and SSE framing are exercised without an OpenAI call.
    """

    @provide(scope=AG2Scope.REQUEST)
    def provide_streamer(self, search: SearchDocumentsUseCase) -> AgUiStreamer:
        return FakeAgUiStreamer(search=search)


@pytest.fixture
def settings() -> Settings:
    # A dummy key keeps Settings valid; the fake streamer never reaches OpenAI.
    return Settings(openai_api_key=SecretStr("sk-test-not-used"))


@pytest.fixture
def container(settings: Settings) -> AsyncContainer:
    return make_async_container(
        SettingsProvider(),
        RepositoryProvider(),
        UseCaseProvider(),
        _FakeStreamerProvider(),
        context={Settings: settings},
        scopes=AG2Scope,
    )


@pytest.fixture
def app(settings: Settings, container: AsyncContainer) -> FastAPI:
    return build_app(settings, container)


@pytest.fixture
def client(app: FastAPI) -> Iterator[TestClient]:
    with TestClient(app) as test_client:
        yield test_client
