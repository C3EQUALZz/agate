from dataclasses import dataclass

from ag_ui_agent.domain.entities import Document
from ag_ui_agent.gateways import DocumentRepository


@dataclass(kw_only=True)
class ListDocumentsRequest:
    limit: int = 20
    offset: int = 0


@dataclass(kw_only=True)
class ListDocumentsResponse:
    documents: list[Document]


class ListDocumentsUseCase:
    def __init__(self, repo: DocumentRepository) -> None:
        self._repo = repo

    async def execute(self, request: ListDocumentsRequest) -> ListDocumentsResponse:
        documents = await self._repo.list(limit=request.limit, offset=request.offset)
        return ListDocumentsResponse(documents=documents)
