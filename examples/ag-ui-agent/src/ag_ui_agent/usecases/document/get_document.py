from dataclasses import dataclass

from ag_ui_agent.domain.entities import Document, DocumentId
from ag_ui_agent.gateways import DocumentRepository
from ag_ui_agent.usecases.errors import DocumentNotFoundError


@dataclass(kw_only=True)
class GetDocumentRequest:
    document_id: DocumentId


@dataclass(kw_only=True)
class GetDocumentResponse:
    document: Document


class GetDocumentUseCase:
    def __init__(self, repo: DocumentRepository) -> None:
        self._repo = repo

    async def execute(self, request: GetDocumentRequest) -> GetDocumentResponse:
        document = await self._repo.get_by_id(request.document_id)
        if document is None:
            raise DocumentNotFoundError(request.document_id)
        return GetDocumentResponse(document=document)
