"""Storage-shaped seed data for the in-memory document workspace.

In the reference this layer holds SQLAlchemy ``Table`` definitions and the
imperative mapper. This example deliberately swaps Postgres for an in-memory
store so it runs with zero external infrastructure (see the project README,
"Why in-memory"). The ``models`` layer therefore owns the concrete initial
state of the store — the equivalent of a migration's seed rows — while the
``gateways`` layer owns the persistence mechanism. The Clean-Architecture layer
direction (``gateways -> models -> domain``) is preserved unchanged.
"""

from datetime import UTC, datetime
from uuid import UUID

from ag_ui_agent.domain.entities import Document, DocumentId

# Fixed UUIDs keep the demo output deterministic across runs.
_README_ID = DocumentId(UUID("00000000-0000-0000-0000-000000000001"))
_NOTES_ID = DocumentId(UUID("00000000-0000-0000-0000-000000000002"))
_EPOCH = datetime(2026, 1, 1, tzinfo=UTC)


def seed_documents() -> list[Document]:
    """Return the initial workspace contents (fresh instances per call)."""
    return [
        Document(
            id=_README_ID,
            name="README.md",
            body="Welcome to the demo workspace. Use search to find documents.",
            created_at=_EPOCH,
        ),
        Document(
            id=_NOTES_ID,
            name="notes.txt",
            body="Remember to rotate the staging API key before the audit.",
            created_at=_EPOCH,
        ),
    ]
