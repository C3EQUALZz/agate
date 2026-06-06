from dataclasses import dataclass

from ag_ui_agent.domain.entities import Document
from ag_ui_agent.gateways import DocumentRepository


@dataclass(kw_only=True)
class SearchDocumentsRequest:
    query: str
    limit: int = 20


@dataclass(kw_only=True)
class SearchDocumentsResponse:
    documents: list[Document]


class SearchDocumentsUseCase:
    """The *safe* capability: read-only full-text search over the workspace.

    This is the tool the demo's Agate allowlist permits.
    """

    def __init__(self, repo: DocumentRepository) -> None:
        self._repo = repo

    async def execute(self, request: SearchDocumentsRequest) -> SearchDocumentsResponse:
        documents = await self._repo.search(query=request.query, limit=request.limit)
        return SearchDocumentsResponse(documents=documents)
