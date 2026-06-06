from dishka import Provider
from dishka_ag2 import AG2Scope

from ag_ui_agent.usecases import (
    DeleteDocumentUseCase,
    GetDocumentUseCase,
    ListDocumentsUseCase,
    SearchDocumentsUseCase,
)


class UseCaseProvider(Provider):
    """Provide all use cases at request scope (one fresh instance per call)."""

    scope = AG2Scope.REQUEST

    def __init__(self) -> None:
        super().__init__()
        self.provide_all(
            DeleteDocumentUseCase,
            GetDocumentUseCase,
            ListDocumentsUseCase,
            SearchDocumentsUseCase,
        )
