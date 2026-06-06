from typing import Protocol

from ag_ui_agent.domain.entities import Document, DocumentId


class DocumentRepository(Protocol):
    """Port for document persistence.

    Use cases depend on this Protocol, never on a concrete store, so the agent's
    behaviour is identical whether documents live in memory, Postgres, or S3.
    """

    async def get_by_id(self, document_id: DocumentId) -> Document | None: ...

    async def list(self, limit: int = 20, offset: int = 0) -> list[Document]: ...

    async def search(self, query: str, limit: int = 20) -> list[Document]: ...

    async def delete(self, document_id: DocumentId) -> bool: ...
