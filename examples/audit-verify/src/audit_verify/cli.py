"""Read Agate's transparency log and summarize what was recorded.

Agate records **every** inspected ``(event, verdict)`` decision as a leaf in an
append-only, RFC 6962 Merkle transparency log (Postgres-backed). This tool reads
that log to show the decisions from the protected-demo were durably recorded —
the "you can prove what happened" half of Agate.

Usage:

    uv run audit-verify
    uv run audit-verify --database-url postgres://agate:agate@localhost:5432/agate
"""

from __future__ import annotations

import argparse
import sys

from audit_verify.config import DEFAULT_DATABASE_URL, Config
from audit_verify.domain import TransparencyLogSummary
from audit_verify.gateways import AuditLogReadError, PostgresAuditLogReader


def run(config: Config) -> int:
    reader = PostgresAuditLogReader(config)
    try:
        summaries = reader.list_summaries()
    except AuditLogReadError as error:
        print(f"database error: {error}", file=sys.stderr)
        print(
            "is Postgres reachable? expose the demo's db port or run inside the "
            "compose network.",
            file=sys.stderr,
        )
        return 1

    if not summaries:
        print("no transparency logs found — has the demo handled a run yet?")
        return 1

    for summary in summaries:
        _print_summary(summary)
    return 0


def _print_summary(summary: TransparencyLogSummary) -> None:
    span = (
        f"leaf_index {summary.min_index}..{summary.max_index}"
        if summary.leaf_count
        else "no leaves yet"
    )
    print(f"transparency log {summary.log_id}")
    print(f"  hash_algo code : {summary.hash_algo_code}")
    print(f"  created (ms)   : {summary.created_at_ms}")
    print(f"  updated (ms)   : {summary.updated_at_ms}")
    print(f"  recorded leaves: {summary.leaf_count}  ({span})")
    contiguous = "yes" if summary.is_contiguous else "NO — gap/reorder detected!"
    print(f"  contiguous     : {contiguous}")
    if summary.sample:
        print("  first leaves   :")
        for leaf in summary.sample:
            print(f"    #{leaf.index:<4} {leaf.digest_hex[:32]}...")
    print()


def parse_args(argv: list[str] | None = None) -> Config:
    parser = argparse.ArgumentParser(description="Inspect Agate's transparency log.")
    parser.add_argument(
        "--database-url",
        default=DEFAULT_DATABASE_URL,
        help=f"PostgreSQL URL (default {DEFAULT_DATABASE_URL})",
    )
    args = parser.parse_args(argv)
    return Config(database_url=args.database_url)


def main() -> None:
    raise SystemExit(run(parse_args()))


if __name__ == "__main__":
    main()
