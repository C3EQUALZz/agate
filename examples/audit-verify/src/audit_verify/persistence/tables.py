"""``Table`` definitions for Agate's audit schema + imperative mapping.

Mirrors PixErase's ``persistence/models/*.py``: declare each pre-existing table
against the shared metadata, then ``map_imperatively`` a plain entity onto it via
a ``map_*`` function. We map onto Agate's schema (``agate-audit/migrations/0001_init.sql``);
we do not own or migrate it.

The ``column_prefix`` trick from PixErase is unnecessary here because our entity
attribute names already match the column names, so the mapping is a direct,
property-by-property map. Query expressions in the gateway reference the
``Table`` columns rather than the (plainly typed) entity attributes — see the
gateway's module docstring.
"""

from __future__ import annotations

from typing import Final

from sqlalchemy import BigInteger, Column, ForeignKey, LargeBinary, SmallInteger, Table, Uuid

from audit_verify.persistence.entities import AuditLeaf, AuditLog
from audit_verify.persistence.registry import mapper_registry

audit_log_table: Final[Table] = Table(
    "audit_log",
    mapper_registry.metadata,
    Column("id", Uuid(as_uuid=True), primary_key=True),
    Column("created_at", BigInteger, nullable=False),
    Column("updated_at", BigInteger, nullable=False),
    Column("hash_algo", SmallInteger, nullable=False),
)

audit_leaf_table: Final[Table] = Table(
    "audit_leaf",
    mapper_registry.metadata,
    Column("log_id", Uuid(as_uuid=True), ForeignKey("audit_log.id"), primary_key=True),
    Column("leaf_index", BigInteger, primary_key=True),
    Column("leaf_hash", LargeBinary, nullable=False),
)


def map_audit_tables() -> None:
    """Imperatively map :class:`AuditLog` / :class:`AuditLeaf` onto their tables.

    Idempotent: re-mapping an already-mapped class would raise, so callers that
    might run more than once (e.g. tests) should guard with
    :func:`audit_tables_mapped`.
    """
    mapper_registry.map_imperatively(
        AuditLog,
        audit_log_table,
        properties={"log_id": audit_log_table.c.id},
    )
    mapper_registry.map_imperatively(AuditLeaf, audit_leaf_table)


def audit_tables_mapped() -> bool:
    """Report whether the imperative mapping has already been configured."""
    return bool(mapper_registry.mappers)
