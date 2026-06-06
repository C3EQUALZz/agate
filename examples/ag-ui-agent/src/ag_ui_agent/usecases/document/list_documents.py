"""The list-documents use case (a safe, read-only capability)."""

from dataclasses import dataclass

from ag_ui_agent.domain.entities import Document
from ag_ui_agent.gateways import DocumentRepository

DEFAULT_LIMIT = 20


@dataclass(kw_only=True)
class ListDocumentsRequest:
    """Inputs for listing documents (newest first)."""

    limit: int = DEFAULT_LIMIT
    offset: int = 0


@dataclass(kw_only=True)
class ListDocumentsResponse:
    """A page of documents."""

    documents: list[Document]


class ListDocumentsUseCase:
    """List documents in the workspace, newest first."""

    def __init__(self, repo: DocumentRepository) -> None:
        self._repo = repo

    async def execute(self, request: ListDocumentsRequest) -> ListDocumentsResponse:
        """Fetch a page of documents from the repository."""
        documents = await self._repo.list(limit=request.limit, offset=request.offset)
        return ListDocumentsResponse(documents=documents)
