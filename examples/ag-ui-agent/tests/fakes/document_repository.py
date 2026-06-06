from ag_ui_agent.domain.entities import Document, DocumentId
from ag_ui_agent.gateways import DocumentRepository


class FakeDocumentRepository(DocumentRepository):
    """A trivial in-memory double used to drive use cases in unit tests."""

    def __init__(self, documents: list[Document] | None = None) -> None:
        self._documents: dict[DocumentId, Document] = {d.id: d for d in (documents or [])}

    async def get_by_id(self, document_id: DocumentId) -> Document | None:
        return self._documents.get(document_id)

    async def list_page(self, limit: int = 20, offset: int = 0) -> list[Document]:
        items = sorted(self._documents.values(), key=lambda d: d.created_at, reverse=True)
        return items[offset : offset + limit]

    async def search(self, query: str, limit: int = 20) -> list[Document]:
        needle = query.casefold()
        matches = [
            d
            for d in self._documents.values()
            if needle in d.name.casefold() or needle in d.body.casefold()
        ]
        matches.sort(key=lambda d: d.created_at, reverse=True)
        return matches[:limit]

    async def delete(self, document_id: DocumentId) -> bool:
        return self._documents.pop(document_id, None) is not None
