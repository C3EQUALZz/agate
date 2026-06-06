from dataclasses import dataclass

from ag_ui_agent.domain.entities import DocumentId
from ag_ui_agent.gateways import DocumentRepository
from ag_ui_agent.usecases.errors import DocumentNotFoundError


@dataclass(kw_only=True)
class DeleteDocumentRequest:
    document_id: DocumentId


class DeleteDocumentUseCase:
    """The *dangerous* capability: destructive, irreversible deletion.

    The agent exposes this as a ``delete_file`` tool. In the protected demo
    Agate's allowlist excludes it, so the proxy denies the tool call before it
    reaches this use case — that is the boundary the example exists to show.
    """

    def __init__(self, repo: DocumentRepository) -> None:
        self._repo = repo

    async def execute(self, request: DeleteDocumentRequest) -> None:
        deleted = await self._repo.delete(request.document_id)
        if not deleted:
            raise DocumentNotFoundError(request.document_id)
