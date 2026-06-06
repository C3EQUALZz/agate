from dishka import BaseScope, Provider, Scope, WithParents

from ag_ui_agent.gateways.db.document import InMemoryDocumentRepository


class RepositoryProvider(Provider):
    """Provide the document repository as an app-scoped singleton.

    The in-memory store must outlive a single request so writes (e.g. a tool
    deleting a document) persist across turns — hence app scope. With a real
    database adapter this would be request-scoped (one session per request), as
    in the reference. ``WithParents`` also binds the ``DocumentRepository``
    Protocol so use cases resolve against the port, not the concrete class.

    The scope is injected because the stub container uses dishka's ``Scope`` and
    the AG2 container uses ``dishka_ag2.AG2Scope`` — two distinct scope families.
    """

    def __init__(self, app_scope: BaseScope = Scope.APP) -> None:
        super().__init__(scope=app_scope)
        self.provide(
            InMemoryDocumentRepository,
            provides=WithParents[InMemoryDocumentRepository],
        )
