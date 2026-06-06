"""The ``Document`` domain entity and its identifier type."""

from dataclasses import dataclass
from datetime import datetime
from typing import NewType
from uuid import UUID

DocumentId = NewType("DocumentId", UUID)


@dataclass(kw_only=True)
class Document:
    """A document in the agent's workspace.

    Pure domain entity: a plain dataclass with no framework, ORM, or I/O
    coupling. The ``models`` layer maps it to storage; the ``gateways`` layer
    persists it; the ``usecases`` layer orchestrates over it.
    """

    id: DocumentId
    name: str
    body: str
    created_at: datetime
