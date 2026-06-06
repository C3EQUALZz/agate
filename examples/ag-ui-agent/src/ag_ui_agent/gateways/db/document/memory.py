"""An in-memory adapter for the ``DocumentRepository`` port."""

from ag_ui_agent.domain.entities import Document, DocumentId
from ag_ui_agent.gateways.db.document.interface import DEFAULT_LIMIT, DocumentRepository
from ag_ui_agent.models import seed_documents


class InMemoryDocumentRepository(DocumentRepository):
    """In-memory adapter for :class:`DocumentRepository`.

    Swaps the reference's ``AlchemyNoteRepository`` for a dict-backed store so
    the example runs without Postgres. The store is seeded from the ``models``
    layer and shared process-wide (registered as an ``APP``-scoped singleton in
    the DI container), so writes from the agent's tools persist across turns.
    """

    def __init__(self) -> None:
        self._documents: dict[DocumentId, Document] = {doc.id: doc for doc in seed_documents()}

    async def get_by_id(self, document_id: DocumentId) -> Document | None:
        """Return the document with this id, or ``None`` if absent."""
        return self._documents.get(document_id)

    async def list_page(self, limit: int = DEFAULT_LIMIT, offset: int = 0) -> list[Document]:
        """Return a page of documents (newest first)."""
        newest_first = sorted(
            self._documents.values(),
            key=lambda doc: doc.created_at,
            reverse=True,
        )
        return newest_first[offset : offset + limit]

    async def search(self, query: str, limit: int = DEFAULT_LIMIT) -> list[Document]:
        """Return documents whose name or body matches ``query`` (case-insensitive)."""
        needle = query.casefold()
        matches = [
            doc
            for doc in self._documents.values()
            if needle in doc.name.casefold() or needle in doc.body.casefold()
        ]
        matches.sort(key=lambda doc: doc.created_at, reverse=True)
        return matches[:limit]

    async def delete(self, document_id: DocumentId) -> bool:
        """Delete the document; return whether it existed."""
        return self._documents.pop(document_id, None) is not None
