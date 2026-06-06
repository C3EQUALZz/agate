"""The search-documents use case (the safe, read-only capability)."""

from dataclasses import dataclass

from ag_ui_agent.domain.entities import Document
from ag_ui_agent.gateways import DocumentRepository

DEFAULT_LIMIT = 20


@dataclass(kw_only=True)
class SearchDocumentsRequest:
    """Inputs for a document search."""

    query: str
    limit: int = DEFAULT_LIMIT


@dataclass(kw_only=True)
class SearchDocumentsResponse:
    """The documents matching a search."""

    documents: list[Document]


class SearchDocumentsUseCase:
    """The *safe* capability: read-only full-text search over the workspace.

    This is the tool the demo's Agate allowlist permits.
    """

    def __init__(self, repo: DocumentRepository) -> None:
        self._repo = repo

    async def execute(self, request: SearchDocumentsRequest) -> SearchDocumentsResponse:
        """Run the search against the repository."""
        documents = await self._repo.search(query=request.query, limit=request.limit)
        return SearchDocumentsResponse(documents=documents)
