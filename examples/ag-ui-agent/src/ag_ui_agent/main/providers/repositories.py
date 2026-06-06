"""Dishka provider wiring the document repository to its port."""

from dishka import Provider, WithParents
from dishka_ag2 import AG2Scope

from ag_ui_agent.gateways.db.document import InMemoryDocumentRepository


class RepositoryProvider(Provider):
    """Provide the document repository as an app-scoped singleton.

    The in-memory store must outlive a single request so writes (e.g. a tool
    deleting a document) persist across turns -- hence app scope. With a real
    database adapter this would be request-scoped (one session per request), as
    in the reference. ``WithParents`` also binds the ``DocumentRepository``
    Protocol so use cases resolve against the port, not the concrete class.
    """

    scope = AG2Scope.APP

    def __init__(self) -> None:
        super().__init__()
        self.provide(
            InMemoryDocumentRepository,
            provides=WithParents[InMemoryDocumentRepository],
        )
