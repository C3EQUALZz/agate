"""Sync transaction/session manager.

Mirrors PixErase's ``alchemy_main_transaction_manager.py`` (a thin wrapper that
owns a session's unit-of-work lifecycle), adapted to **sync** SQLAlchemy and to a
read-only tool. Instead of exposing ``commit``/``flush`` separately, it offers a
single context manager that opens a session inside a transaction (``begin()``) and
guarantees the session is closed afterwards. Repositories receive an open session
from it and never touch the factory directly.
"""

from __future__ import annotations

from contextlib import contextmanager
from typing import TYPE_CHECKING, Final

from sqlalchemy.exc import SQLAlchemyError

if TYPE_CHECKING:
    from collections.abc import Iterator

    from sqlalchemy.orm import Session, sessionmaker


class TransactionError(RuntimeError):
    """Raised when a unit of work cannot be opened or completed."""


class SqlAlchemyTransactionManager:
    """Own the session/transaction lifecycle for a single unit of work."""

    def __init__(self, session_factory: sessionmaker[Session]) -> None:
        self._session_factory: Final[sessionmaker[Session]] = session_factory

    @contextmanager
    def begin(self) -> Iterator[Session]:
        """Yield a session wrapped in a transaction; always close it after.

        ``Session.begin()`` commits on a clean exit and rolls back on error.
        This is read-only, so the commit is a harmless no-op; either way the
        ``finally`` closes the session and releases the connection.
        """
        session = self._session_factory()
        try:
            with session.begin():
                yield session
        except SQLAlchemyError as error:
            raise TransactionError(str(error)) from error
        finally:
            session.close()
