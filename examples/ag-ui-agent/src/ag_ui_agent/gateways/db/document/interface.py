"""The ``DocumentRepository`` persistence port (a Protocol)."""

from typing import Protocol

from ag_ui_agent.domain.entities import Document, DocumentId

DEFAULT_LIMIT = 20


class DocumentRepository(Protocol):
    """Port for document persistence.

    Use cases depend on this Protocol, never on a concrete store, so the agent's
    behaviour is identical whether documents live in memory, Postgres, or S3.
    """

    async def get_by_id(self, document_id: DocumentId) -> Document | None:
        """Return the document with this id, or ``None`` if absent."""
        ...

    async def list(self, limit: int = DEFAULT_LIMIT, offset: int = 0) -> list[Document]:
        """Return a page of documents (newest first)."""
        ...

    async def search(self, query: str, limit: int = DEFAULT_LIMIT) -> list[Document]:
        """Return documents whose name or body matches ``query``."""
        ...

    async def delete(self, document_id: DocumentId) -> bool:
        """Delete the document; return whether it existed."""
        ...
