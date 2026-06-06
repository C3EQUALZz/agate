"""AG2 tools backed by Dishka-injected use cases.

These are imported only by the real-AG2 backend (``backends/ag2.py``); the
``autogen`` / ``dishka_ag2`` imports therefore stay inside ``api.agent``, as the
import-linter contract requires. Each tool looks like a plain Dishka handler:
``@tool @inject async def ...(uc: FromDishka[UseCase], ...)``. ``dishka-ag2``'s
``DishkaAsyncMiddleware`` opens an ``AG2Scope.REQUEST`` child container before the
tool runs and resolves the ``FromDishka[...]`` parameters out of it.
"""

from dataclasses import dataclass
from datetime import datetime
from uuid import UUID

from autogen.beta import Toolkit, tool
from dishka_ag2 import FromDishka, inject

from ag_ui_agent.domain.entities import Document, DocumentId
from ag_ui_agent.usecases import (
    DeleteDocumentRequest,
    DeleteDocumentUseCase,
    ListDocumentsRequest,
    ListDocumentsUseCase,
    SearchDocumentsRequest,
    SearchDocumentsUseCase,
)


@dataclass(slots=True, frozen=True)
class DocumentToolResult:
    id: UUID
    name: str
    body: str
    created_at: datetime

    @classmethod
    def from_entity(cls, document: Document) -> "DocumentToolResult":
        return cls(
            id=document.id,
            name=document.name,
            body=document.body,
            created_at=document.created_at,
        )


@tool
@inject
async def search_documents(
    uc: FromDishka[SearchDocumentsUseCase],
    query: str,
    limit: int = 20,
) -> list[DocumentToolResult]:
    """Search the workspace for documents matching a query (safe, read-only).

    Args:
        query: text to match against document names and bodies.
        limit: maximum number of results.
    """
    response = await uc.execute(SearchDocumentsRequest(query=query, limit=limit))
    return [DocumentToolResult.from_entity(d) for d in response.documents]


@tool
@inject
async def list_documents(
    uc: FromDishka[ListDocumentsUseCase],
    limit: int = 20,
) -> list[DocumentToolResult]:
    """List documents in the workspace (safe, read-only).

    Args:
        limit: maximum number of documents to return (newest first).
    """
    response = await uc.execute(ListDocumentsRequest(limit=limit))
    return [DocumentToolResult.from_entity(d) for d in response.documents]


@tool
@inject
async def delete_file(
    uc: FromDishka[DeleteDocumentUseCase],
    document_id: str,
) -> str:
    """Permanently delete a document by id (DANGEROUS, destructive).

    Args:
        document_id: UUID of the document to delete.
    """
    await uc.execute(DeleteDocumentRequest(document_id=DocumentId(UUID(document_id))))
    return "deleted"


def documents_toolkit() -> Toolkit:
    return Toolkit(search_documents, list_documents, delete_file)
