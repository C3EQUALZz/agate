"""Dishka provider exposing the application ``Settings``."""

from dishka import Provider
from dishka_ag2 import AG2Scope

from ag_ui_agent.config import Settings


class SettingsProvider(Provider):
    """Expose the app ``Settings`` (supplied via container context) to DI."""

    scope = AG2Scope.APP

    def __init__(self) -> None:
        super().__init__()
        self.from_context(provides=Settings, scope=AG2Scope.APP)
