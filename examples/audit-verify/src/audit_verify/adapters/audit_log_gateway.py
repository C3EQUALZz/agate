"""Port + SQLAlchemy adapter for reading Agate's transparency log.

Schema (``crates/agate-audit/migrations/0001_init.sql``):

  * ``audit_log``  — ``id`` (UUID), ``created_at`` / ``updated_at`` (Unix ms),
    ``hash_algo`` (SMALLINT, the epoch hash-algorithm code).
  * ``audit_leaf`` — ``log_id`` (UUID), ``leaf_index`` (BIGINT, 0-based,
    monotonic), ``leaf_hash`` (BYTEA, the bytes hashed into the Merkle tree).

Mirrors PixErase's ``alchemy_*_query_gateway.py``: a query gateway that receives
an open session (here, via the transaction manager) and queries through the ORM
over the imperatively mapped entities. No hand-written row tuples — aggregates
are typed ``select`` statements over the mapped columns, and leaves come back as
:class:`AuditLeaf` instances.

Query expressions reference the ``Table`` columns (``audit_log_table.c.*``)
rather than the plain entity attributes: under *imperative* mapping the entity
attributes are typed as their Python values (``int``/``bytes``), so the columns
are what gives ``where``/``order_by`` their typed SQL-expression semantics.

VERIFY: this reads the tables directly because the Agate build targeted here
exposes the log via Postgres, not (yet) via an HTTP inclusion-proof endpoint.
The leaf stores a *hash*, not the decoded event — digests are shown, not
payloads. If your Agate build ships an inclusion-proof API, prefer it.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Final, Protocol
from uuid import UUID

from sqlalchemy import func, select

from audit_verify.adapters.transaction_manager import (
    SqlAlchemyTransactionManager,
    TransactionError,
)
from audit_verify.domain import LeafSample, TransparencyLogSummary
from audit_verify.persistence import AuditLeaf, AuditLog
from audit_verify.persistence.tables import audit_leaf_table, audit_log_table

if TYPE_CHECKING:
    from sqlalchemy.orm import Session

    from audit_verify.config import Config


class AuditLogReadError(RuntimeError):
    """Raised when the transparency log cannot be read."""


class AuditLogReader(Protocol):
    """Port: read summarized transparency logs from some store."""

    def list_summaries(self) -> list[TransparencyLogSummary]:
        """Return a summary of every transparency log in the store."""
        ...


class SqlAlchemyAuditLogReader(AuditLogReader):
    """Adapter: read the transparency log via SQLAlchemy over the audit tables."""

    def __init__(
        self,
        transaction_manager: SqlAlchemyTransactionManager,
        config: Config,
    ) -> None:
        self._transactions: Final = transaction_manager
        self._sample_leaves: Final[int] = config.sample_leaves

    def list_summaries(self) -> list[TransparencyLogSummary]:
        """Return a summary of every transparency log in the database."""
        try:
            with self._transactions.begin() as session:
                query = select(AuditLog).order_by(audit_log_table.c.created_at)
                logs = session.scalars(query).all()
                return [self._summarize(session, log) for log in logs]
        except TransactionError as error:
            raise AuditLogReadError(str(error)) from error

    def _summarize(self, session: Session, log: AuditLog) -> TransparencyLogSummary:
        leaf_of_log = audit_leaf_table.c.log_id == log.log_id
        index_col = audit_leaf_table.c.leaf_index
        # Three typed scalar aggregates rather than one multi-column Row: each
        # `session.scalar` returns a concrete `int | None`, so no `Any` from
        # tuple-unpacking a `Row` leaks into the domain summary.
        count_query = select(func.count()).where(leaf_of_log)
        count = session.scalar(count_query) or 0
        min_index = session.scalar(select(func.min(index_col)).where(leaf_of_log))
        max_index = session.scalar(select(func.max(index_col)).where(leaf_of_log))
        return TransparencyLogSummary(
            log_id=log.log_id,
            created_at_ms=log.created_at,
            updated_at_ms=log.updated_at,
            hash_algo_code=log.hash_algo,
            leaf_count=count,
            min_index=min_index,
            max_index=max_index,
            sample=self._sample(session, log.log_id),
        )

    def _sample(self, session: Session, log_id: UUID) -> tuple[LeafSample, ...]:
        query = (
            select(AuditLeaf)
            .where(audit_leaf_table.c.log_id == log_id)
            .order_by(audit_leaf_table.c.leaf_index)
            .limit(self._sample_leaves)
        )
        leaves = session.scalars(query).all()
        return tuple(
            LeafSample(index=leaf.leaf_index, leaf_hash=bytes(leaf.leaf_hash)) for leaf in leaves
        )
