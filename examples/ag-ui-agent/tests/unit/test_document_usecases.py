from datetime import UTC, datetime
from uuid import UUID, uuid4

import pytest

from ag_ui_agent.domain.entities import Document, DocumentId
from ag_ui_agent.usecases.document import (
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
from tests.fakes.document_repository import FakeDocumentRepository


def _doc(name: str, body: str = "", *, day: int = 1) -> Document:
    return Document(
        id=DocumentId(uuid4()),
        name=name,
        body=body,
        created_at=datetime(2026, 1, day, tzinfo=UTC),
    )


@pytest.mark.asyncio
async def test_search_matches_name_and_body() -> None:
    repo = FakeDocumentRepository(
        [_doc("readme.md", "hello"), _doc("notes.txt", "secret key")]
    )
    uc = SearchDocumentsUseCase(repo=repo)

    by_name = await uc.execute(SearchDocumentsRequest(query="readme"))
    by_body = await uc.execute(SearchDocumentsRequest(query="KEY"))

    assert [d.name for d in by_name.documents] == ["readme.md"]
    assert [d.name for d in by_body.documents] == ["notes.txt"]


@pytest.mark.asyncio
async def test_list_returns_newest_first() -> None:
    repo = FakeDocumentRepository([_doc("a", day=1), _doc("b", day=2)])
    uc = ListDocumentsUseCase(repo=repo)

    response = await uc.execute(ListDocumentsRequest(limit=10))

    assert [d.name for d in response.documents] == ["b", "a"]


@pytest.mark.asyncio
async def test_get_raises_when_missing() -> None:
    uc = GetDocumentUseCase(repo=FakeDocumentRepository())
    with pytest.raises(DocumentNotFoundError):
        await uc.execute(GetDocumentRequest(document_id=DocumentId(uuid4())))


@pytest.mark.asyncio
async def test_delete_removes_document() -> None:
    doc = _doc("trash.txt")
    repo = FakeDocumentRepository([doc])
    uc = DeleteDocumentUseCase(repo=repo)

    await uc.execute(DeleteDocumentRequest(document_id=doc.id))

    assert await repo.get_by_id(doc.id) is None


@pytest.mark.asyncio
async def test_delete_raises_when_missing() -> None:
    uc = DeleteDocumentUseCase(repo=FakeDocumentRepository())
    with pytest.raises(DocumentNotFoundError):
        await uc.execute(
            DeleteDocumentRequest(
                document_id=DocumentId(UUID("00000000-0000-0000-0000-0000000000ff"))
            )
        )
