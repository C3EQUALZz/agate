"""Integration test for the SQLAlchemy reader over the imperative mapping.

The production target is Agate's Postgres, but the mapped ``Table``s are
dialect-agnostic, so we point the same imperative mapping at an in-memory SQLite
database, create the schema there, insert rows, and assert that the reader builds
the expected :class:`TransparencyLogSummary` (including the contiguity signal and
the leaf sample). This exercises the transaction manager + query gateway without
needing a Postgres.
"""

from collections.abc import Iterator
from uuid import UUID

import pytest
from sqlalchemy import create_engine, insert
from sqlalchemy.orm import Session, sessionmaker

from audit_verify.adapters import SqlAlchemyAuditLogReader, SqlAlchemyTransactionManager
from audit_verify.config import Config
from audit_verify.persistence.registry import mapper_registry
from audit_verify.persistence.tables import (
    audit_leaf_table,
    audit_log_table,
    audit_tables_mapped,
    map_audit_tables,
)

_LOG_ID = UUID("3f6c1e2a-0000-0000-0000-000000000000")

# Configure the imperative mapping once, before any query builds a statement.
if not audit_tables_mapped():
    map_audit_tables()


@pytest.fixture
def session_factory() -> Iterator[sessionmaker[Session]]:
    engine = create_engine("sqlite+pysqlite:///:memory:")
    mapper_registry.metadata.create_all(engine)
    try:
        yield sessionmaker(bind=engine, expire_on_commit=False)
    finally:
        engine.dispose()


def _seed(factory: sessionmaker[Session], leaf_count: int) -> None:
    with factory.begin() as session:
        session.execute(
            insert(audit_log_table).values(
                id=_LOG_ID,
                created_at=1,
                updated_at=2,
                hash_algo=1,
            ),
        )
        for index in range(leaf_count):
            session.execute(
                insert(audit_leaf_table).values(
                    log_id=_LOG_ID,
                    leaf_index=index,
                    leaf_hash=bytes([index, 0x2B, 0x1C]),
                ),
            )


def test_reader_summarizes_contiguous_log(session_factory: sessionmaker[Session]) -> None:
    _seed(session_factory, leaf_count=3)
    reader = SqlAlchemyAuditLogReader(SqlAlchemyTransactionManager(session_factory), Config())

    summaries = reader.list_summaries()

    assert len(summaries) == 1
    summary = summaries[0]
    assert summary.log_id == _LOG_ID
    assert summary.hash_algo_code == 1
    assert summary.leaf_count == 3
    assert summary.min_index == 0
    assert summary.max_index == 2
    assert summary.is_contiguous
    assert [leaf.index for leaf in summary.sample] == [0, 1, 2]
    assert summary.sample[0].digest_hex == "002b1c"


def test_reader_handles_empty_log(session_factory: sessionmaker[Session]) -> None:
    _seed(session_factory, leaf_count=0)
    reader = SqlAlchemyAuditLogReader(SqlAlchemyTransactionManager(session_factory), Config())

    summary = reader.list_summaries()[0]

    assert summary.leaf_count == 0
    assert summary.min_index is None
    assert summary.max_index is None
    assert summary.is_contiguous
    assert summary.sample == ()


def test_reader_respects_sample_limit(session_factory: sessionmaker[Session]) -> None:
    _seed(session_factory, leaf_count=5)
    config = Config(sample_leaves=2)
    reader = SqlAlchemyAuditLogReader(SqlAlchemyTransactionManager(session_factory), config)

    summary = reader.list_summaries()[0]

    assert summary.leaf_count == 5
    assert [leaf.index for leaf in summary.sample] == [0, 1]
