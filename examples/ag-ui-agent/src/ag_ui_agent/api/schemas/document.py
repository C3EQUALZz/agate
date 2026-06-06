from datetime import datetime
from uuid import UUID

from pydantic import BaseModel, Field

from ag_ui_agent.domain.entities import Document


class DocumentRead(BaseModel):
    id: UUID
    name: str
    body: str
    created_at: datetime

    @classmethod
    def from_entity(cls, document: Document) -> "DocumentRead":
        return cls(
            id=document.id,
            name=document.name,
            body=document.body,
            created_at=document.created_at,
        )


class DocumentList(BaseModel):
    documents: list[DocumentRead]
    total: int


class SearchQuery(BaseModel):
    query: str = Field(min_length=1, max_length=512)
    limit: int = Field(default=20, ge=1, le=200)
