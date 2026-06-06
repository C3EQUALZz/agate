"""Engine + session-factory provider.

Mirrors PixErase's ``persistence/provider.py`` (engine + ``sessionmaker``), but
**sync**: the CLI is synchronous, so a sync ``Engine`` / ``Session`` is the
cleaner fit (no event loop, no async driver). It still uses the ``psycopg`` v3
driver under the hood, via SQLAlchemy's ``postgresql+psycopg`` dialect.

Building the engine also configures the imperative mapping (once), so importers
get a session factory whose ORM queries over :class:`AuditLog` / :class:`AuditLeaf`
are ready to run.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from sqlalchemy import create_engine
from sqlalchemy.engine import make_url
from sqlalchemy.orm import sessionmaker

from audit_verify.persistence.tables import audit_tables_mapped, map_audit_tables

if TYPE_CHECKING:
    from sqlalchemy import URL
    from sqlalchemy.orm import Session


def _normalize_url(database_url: str) -> URL:
    """Force the ``postgresql+psycopg`` dialect onto a libpq-style URL.

    Agate (and the README) use ``postgres://…``; SQLAlchemy needs an explicit
    DBAPI, so we pin the ``psycopg`` (v3) driver rather than the legacy default.
    """
    url = make_url(database_url)
    if url.drivername in {"postgres", "postgresql"}:
        return url.set(drivername="postgresql+psycopg")
    return url


def build_session_factory(
    database_url: str,
    *,
    connect_timeout: int,
) -> sessionmaker[Session]:
    """Build a sync ``Session`` factory bound to ``database_url``.

    Configures the imperative mapping on first use. ``expire_on_commit`` is left
    off — this is a read-only tool, so detached entities outliving the session
    are fine.
    """
    if not audit_tables_mapped():
        map_audit_tables()
    engine = create_engine(
        _normalize_url(database_url),
        pool_pre_ping=True,
        connect_args={"connect_timeout": connect_timeout},
    )
    return sessionmaker(bind=engine, expire_on_commit=False)
