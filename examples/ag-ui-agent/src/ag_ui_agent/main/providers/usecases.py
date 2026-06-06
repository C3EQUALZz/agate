from dishka import BaseScope, Provider, Scope

from ag_ui_agent.usecases import (
    DeleteDocumentUseCase,
    GetDocumentUseCase,
    ListDocumentsUseCase,
    SearchDocumentsUseCase,
)


class UseCaseProvider(Provider):
    """Provide all use cases at request scope (one fresh instance per call).

    The request scope is injected so the same provider works in the stub
    container (dishka ``Scope.REQUEST``) and the AG2 container
    (``dishka_ag2.AG2Scope.REQUEST``).
    """

    def __init__(self, request_scope: BaseScope = Scope.REQUEST) -> None:
        super().__init__(scope=request_scope)
        self.provide_all(
            DeleteDocumentUseCase,
            GetDocumentUseCase,
            ListDocumentsUseCase,
            SearchDocumentsUseCase,
        )
