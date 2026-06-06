"""DI wiring for the offline stub AG-UI streamer.

The AG2-backed streamer lives in ``agent_ag2`` and is imported lazily (only when
``backend=ag2``), so the stub path works with the ``ag2`` extra uninstalled.
"""

from dishka import Provider, Scope, provide

from ag_ui_agent.api.agent import AgUiStreamer
from ag_ui_agent.api.agent.backends import StubAgUiStreamer
from ag_ui_agent.usecases import SearchDocumentsUseCase


class StubAgentProvider(Provider):
    """Provide the offline scripted streamer (no autogen, no API key)."""

    scope = Scope.REQUEST

    @provide
    def provide_streamer(self, search: SearchDocumentsUseCase) -> AgUiStreamer:
        return StubAgUiStreamer(search=search)
