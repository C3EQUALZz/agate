"""REST view over the same use cases the agent's tools call.

Useful for smoke-testing the workspace without driving the LLM, and to show that
HTTP handlers and agent tools share one Dishka container and the same use cases.
"""

from typing import Annotated
from uuid import UUID

from dishka.integrations.fastapi import FromDishka, inject
from fastapi import APIRouter, HTTPException, Path, Query, status

from ag_ui_agent.api.schemas.document import DocumentList, DocumentRead
from ag_ui_agent.domain.entities import DocumentId
from ag_ui_agent.usecases import (
    DeleteDocumentRequest,
    DeleteDocumentUseCase,
    GetDocumentRequest,
    GetDocumentUseCase,
    ListDocumentsRequest,
    ListDocumentsUseCase,
    SearchDocumentsRequest,
    SearchDocumentsUseCase,
)
from ag_ui_agent.usecases.errors import DocumentNotFoundError

router = APIRouter(prefix="/documents", tags=["documents"])


@router.get("")
@inject
async def list_documents(
    use_case: FromDishka[ListDocumentsUseCase],
    limit: Annotated[int, Query(ge=1, le=200)] = 50,
    offset: Annotated[int, Query(ge=0)] = 0,
) -> DocumentList:
    response = await use_case.execute(ListDocumentsRequest(limit=limit, offset=offset))
    return DocumentList(
        documents=[DocumentRead.from_entity(d) for d in response.documents],
        total=len(response.documents),
    )


@router.get("/search")
@inject
async def search_documents(
    use_case: FromDishka[SearchDocumentsUseCase],
    query: Annotated[str, Query(min_length=1, max_length=512)],
    limit: Annotated[int, Query(ge=1, le=200)] = 50,
) -> DocumentList:
    response = await use_case.execute(SearchDocumentsRequest(query=query, limit=limit))
    return DocumentList(
        documents=[DocumentRead.from_entity(d) for d in response.documents],
        total=len(response.documents),
    )


@router.get("/{document_id}")
@inject
async def get_document(
    document_id: Annotated[UUID, Path()],
    use_case: FromDishka[GetDocumentUseCase],
) -> DocumentRead:
    try:
        response = await use_case.execute(GetDocumentRequest(document_id=DocumentId(document_id)))
    except DocumentNotFoundError as e:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail=str(e)) from e
    return DocumentRead.from_entity(response.document)


@router.delete("/{document_id}", status_code=status.HTTP_204_NO_CONTENT)
@inject
async def delete_document(
    document_id: Annotated[UUID, Path()],
    use_case: FromDishka[DeleteDocumentUseCase],
) -> None:
    try:
        await use_case.execute(DeleteDocumentRequest(document_id=DocumentId(document_id)))
    except DocumentNotFoundError as e:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail=str(e)) from e
