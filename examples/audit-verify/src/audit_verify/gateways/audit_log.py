"""Port + psycopg adapter for reading Agate's transparency log.

Schema (``crates/agate-audit/migrations/0001_init.sql``):

  * ``audit_log``  — ``id`` (UUID), ``created_at`` / ``updated_at`` (Unix ms),
    ``hash_algo`` (SMALLINT, the epoch hash-algorithm code).
  * ``audit_leaf`` — ``log_id`` (UUID), ``leaf_index`` (BIGINT, 0-based,
    monotonic), ``leaf_hash`` (BYTEA, the bytes hashed into the Merkle tree).

VERIFY: this reads the tables directly because the Agate build targeted here
exposes the log via Postgres, not (yet) via an HTTP inclusion-proof endpoint.
The leaf stores a *hash*, not the decoded event — digests are shown, not
payloads. If your Agate build ships an inclusion-proof API, prefer it.
"""

from __future__ import annotations

from typing import Any, Protocol
from uuid import UUID

import psycopg

from audit_verify.config import Config
from audit_verify.domain import LeafSample, TransparencyLogSummary


class AuditLogReadError(RuntimeError):
    """Raised when the transparency log cannot be read."""


class AuditLogReader(Protocol):
    """Port: read summarized transparency logs from some store."""

    def list_summaries(self) -> list[TransparencyLogSummary]:
        """Return a summary of every transparency log in the store."""
        ...


class PostgresAuditLogReader(AuditLogReader):
    """Adapter: read the transparency log straight from Agate's Postgres."""

    def __init__(self, config: Config) -> None:
        self._config = config

    def list_summaries(self) -> list[TransparencyLogSummary]:
        """Return a summary of every transparency log in the database."""
        try:
            with psycopg.connect(
                self._config.database_url,
                connect_timeout=self._config.connect_timeout,
            ) as conn:
                return [self._summarize(conn, row) for row in _fetch_logs(conn)]
        except psycopg.Error as error:
            raise AuditLogReadError(str(error)) from error

    def _summarize(
        self,
        conn: psycopg.Connection,
        row: tuple[Any, ...],
    ) -> TransparencyLogSummary:
        log_id, created_at, updated_at, hash_algo = row
        count, min_index, max_index = _fetch_leaf_bounds(conn, log_id)
        return TransparencyLogSummary(
            log_id=log_id if isinstance(log_id, UUID) else UUID(str(log_id)),
            created_at_ms=created_at,
            updated_at_ms=updated_at,
            hash_algo_code=hash_algo,
            leaf_count=count,
            min_index=min_index,
            max_index=max_index,
            sample=_fetch_leaf_sample(conn, log_id, self._config.sample_leaves),
        )


def _fetch_logs(conn: psycopg.Connection) -> list[tuple[Any, ...]]:
    """Return ``(id, created_at, updated_at, hash_algo)`` for every log."""
    with conn.cursor() as cur:
        cur.execute(
            "SELECT id, created_at, updated_at, hash_algo FROM audit_log ORDER BY created_at"
        )
        return cur.fetchall()


def _fetch_leaf_bounds(
    conn: psycopg.Connection,
    log_id: object,
) -> tuple[int, int | None, int | None]:
    """Return ``(leaf_count, min_index, max_index)`` for one log."""
    with conn.cursor() as cur:
        cur.execute(
            "SELECT count(*), min(leaf_index), max(leaf_index) FROM audit_leaf WHERE log_id = %s",
            (log_id,),
        )
        return cur.fetchone() or (0, None, None)


def _fetch_leaf_sample(
    conn: psycopg.Connection,
    log_id: object,
    limit: int,
) -> tuple[LeafSample, ...]:
    """Return the first ``limit`` recorded leaves of one log."""
    with conn.cursor() as cur:
        cur.execute(
            "SELECT leaf_index, leaf_hash FROM audit_leaf "
            "WHERE log_id = %s ORDER BY leaf_index LIMIT %s",
            (log_id, limit),
        )
        return tuple(
            LeafSample(index=index, leaf_hash=bytes(leaf_hash))
            for index, leaf_hash in cur.fetchall()
        )
