"""Shared SQLAlchemy ``MetaData`` and imperative ``registry``.

Mirrors PixErase's ``persistence/models/base.py``: a single ``MetaData`` (with a
constraint naming convention) and a ``registry`` bound to it. Because this tool
does **not** own the schema (Agate's ``agate-audit`` migrations do), we only use
this metadata to *describe* the existing tables for imperative mapping — never to
``create_all`` against Agate's Postgres.
"""

from __future__ import annotations

from typing import Final

from sqlalchemy import MetaData
from sqlalchemy.orm import registry

metadata: Final[MetaData] = MetaData(
    naming_convention={
        "ix": "ix_%(column_0_label)s",
        "uq": "uq_%(table_name)s_%(column_0_name)s",
        "ck": "ck_%(table_name)s_%(constraint_name)s",
        "fk": "fk_%(table_name)s_%(column_0_name)s_%(referred_table_name)s",
        "pk": "pk_%(table_name)s",
    },
)

mapper_registry: Final[registry] = registry(metadata=metadata)
