from collections.abc import Iterator

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

from ag_ui_agent.config import AgentBackend, Settings
from ag_ui_agent.main.entrypoint import create_app


@pytest.fixture
def settings() -> Settings:
    # The integration suite exercises the offline stub backend only: no API key,
    # no autogen, no external infrastructure.
    return Settings(backend=AgentBackend.STUB)


@pytest.fixture
def app(settings: Settings) -> FastAPI:
    return create_app(settings=settings)


@pytest.fixture
def client(app: FastAPI) -> Iterator[TestClient]:
    with TestClient(app) as c:
        yield c
