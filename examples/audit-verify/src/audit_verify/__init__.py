"""Inspect Agate's tamper-evident transparency log.

Agate records every inspected ``(event, verdict)`` as a leaf in an RFC 6962
Merkle transparency log, persisted in Postgres. This tool reads that log to show
that the decisions from the protected demo were durably recorded.
"""

__all__ = ["__version__"]

__version__ = "0.1.0"
