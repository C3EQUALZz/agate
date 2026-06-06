from ag_ui_agent.domain.entities import Document, DocumentId
from ag_ui_agent.gateways.db.document.interface import DocumentRepository
from ag_ui_agent.models import seed_documents


class InMemoryDocumentRepository(DocumentRepository):
    """In-memory adapter for :class:`DocumentRepository`.

    Swaps the reference's ``AlchemyNoteRepository`` for a dict-backed store so
    the example runs without Postgres. The store is seeded from the ``models``
    layer and shared process-wide (registered as an ``APP``-scoped singleton in
    the DI container), so writes from the agent's tools persist across turns.
    """

    def __init__(self) -> None:
        self._documents: dict[DocumentId, Document] = {
            doc.id: doc for doc in seed_documents()
        }

    async def get_by_id(self, document_id: DocumentId) -> Document | None:
        return self._documents.get(document_id)

    async def list(self, limit: int = 20, offset: int = 0) -> list[Document]:
        items = sorted(self._documents.values(), key=lambda d: d.created_at, reverse=True)
        return items[offset : offset + limit]

    async def search(self, query: str, limit: int = 20) -> list[Document]:
        needle = query.casefold()
        matches = [
            doc
            for doc in self._documents.values()
            if needle in doc.name.casefold() or needle in doc.body.casefold()
        ]
        matches.sort(key=lambda d: d.created_at, reverse=True)
        return matches[:limit]

    async def delete(self, document_id: DocumentId) -> bool:
        return self._documents.pop(document_id, None) is not None
