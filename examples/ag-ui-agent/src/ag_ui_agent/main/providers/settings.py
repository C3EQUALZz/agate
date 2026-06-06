from dishka import BaseScope, Provider, Scope

from ag_ui_agent.config import Settings


class SettingsProvider(Provider):
    """Expose the app ``Settings`` (supplied via container context) to DI.

    The app scope is injected so the same provider works in the stub container
    (dishka ``Scope.APP``) and the AG2 container (``dishka_ag2.AG2Scope.APP``).
    """

    def __init__(self, app_scope: BaseScope = Scope.APP) -> None:
        super().__init__(scope=app_scope)
        self.from_context(provides=Settings, scope=app_scope)
