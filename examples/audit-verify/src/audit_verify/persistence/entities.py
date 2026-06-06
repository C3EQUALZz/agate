"""Persistence entities mapped imperatively onto Agate's audit tables.

These are plain Python classes with no SQLAlchemy base class and no ORM
decorators — exactly the shape imperative (classical) mapping wants. The
``Table``/``map_imperatively`` wiring lives in
:mod:`audit_verify.persistence.tables`, which instruments these classes (and
supplies a default constructor) at mapping time.

The annotations below are the entities' typed shape: imperative mapping populates
exactly these attributes, and giving them explicit types is what lets the gateway
read ``log.log_id`` / ``leaf.leaf_index`` without any ``Any`` creeping in.

They are *persistence* types, distinct from the pure domain summaries in
:mod:`audit_verify.domain`. The gateway reads these and builds the domain
``TransparencyLogSummary`` from them.
"""

from __future__ import annotations

from uuid import UUID


class AuditLog:
    """One transparency log (``audit_log`` row).

    ``log_id`` maps to the ``id`` column (the attribute is renamed to avoid
    shadowing the ``id`` builtin); the rest match their columns by name.
    """

    log_id: UUID
    created_at: int
    updated_at: int
    hash_algo: int


class AuditLeaf:
    """One recorded ``(event, verdict)`` decision (``audit_leaf`` row)."""

    log_id: UUID
    leaf_index: int
    leaf_hash: bytes
