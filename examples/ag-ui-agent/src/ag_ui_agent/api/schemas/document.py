"""Pydantic schemas for the REST views over documents."""

from datetime import datetime
from uuid import UUID

from pydantic import BaseModel, Field

from ag_ui_agent.domain.entities import Document

_MAX_QUERY_LEN = 512
_MAX_LIMIT = 200
_DEFAULT_LIMIT = 20


class DocumentRead(BaseModel):
    """A single document as returned by the REST API."""

    id: UUID
    name: str
    body: str
    created_at: datetime

    @classmethod
    def from_entity(cls, document: Document) -> "DocumentRead":
        """Project a domain ``Document`` into the REST schema."""
        return cls(
            id=document.id,
            name=document.name,
            body=document.body,
            created_at=document.created_at,
        )


class DocumentList(BaseModel):
    """A page of documents plus the count returned."""

    documents: list[DocumentRead]
    total: int


class SearchQuery(BaseModel):
    """Validated query parameters for a document search."""

    query: str = Field(min_length=1, max_length=_MAX_QUERY_LEN)
    limit: int = Field(default=_DEFAULT_LIMIT, ge=1, le=_MAX_LIMIT)
