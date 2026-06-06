"""Read Agate's transparency log from Postgres and summarize what was recorded.

Agate's audit context (RFC 6962 Merkle transparency log) persists two tables
(see ``crates/agate-audit/migrations/0001_init.sql``):

  * ``audit_log``  — one row per log: ``id`` (UUID), ``created_at`` /
    ``updated_at`` (Unix ms), ``hash_algo`` (the epoch hash algorithm code).
  * ``audit_leaf`` — one row per appended leaf: ``log_id``, ``leaf_index``
    (0-based, monotonic), ``leaf_hash`` (the bytes hashed into the Merkle tree).

Each leaf is one recorded ``(event, verdict)`` decision. The append-only,
gapless ``leaf_index`` sequence is the tamper-evidence: a removed or reordered
decision breaks the Merkle tree head.

Usage:

    uv run audit-verify
    uv run audit-verify --database-url postgres://agate:agate@localhost:5432/agate

By default it targets the protected-demo's Postgres. That Postgres is not
published to the host by default; either add ``ports: ["5432:5432"]`` to the
``db`` service in ``../protected-demo/docker-compose.yml``, or run this inside
the compose network.

VERIFY: this reads the schema directly (stable migration above). Agate does not
yet expose an HTTP "inclusion proof" endpoint in the docs we built against; when
it does, prefer that API over reading tables. The leaf *hash* is shown, not the
decoded event — the event payload is hashed into the leaf, not stored verbatim.
"""

from __future__ import annotations

import argparse
import sys

import psycopg

DEFAULT_DATABASE_URL = "postgres://agate:agate@localhost:5432/agate"


def summarize(database_url: str) -> int:
    try:
        with psycopg.connect(database_url, connect_timeout=5) as conn:
            logs = _fetch_logs(conn)
            if not logs:
                print("no transparency logs found — has the demo handled a run yet?")
                return 1
            for log_id, created_at, updated_at, hash_algo in logs:
                _print_log(conn, log_id, created_at, updated_at, hash_algo)
    except psycopg.Error as error:
        print(f"database error: {error}", file=sys.stderr)
        print(
            "is Postgres reachable? expose the demo's db port or run inside the "
            "compose network.",
            file=sys.stderr,
        )
        return 1
    return 0


def _fetch_logs(conn: psycopg.Connection) -> list[tuple]:
    with conn.cursor() as cur:
        cur.execute(
            "SELECT id, created_at, updated_at, hash_algo "
            "FROM audit_log ORDER BY created_at"
        )
        return cur.fetchall()


def _print_log(
    conn: psycopg.Connection, log_id, created_at: int, updated_at: int, hash_algo: int
) -> None:
    with conn.cursor() as cur:
        cur.execute(
            "SELECT count(*), min(leaf_index), max(leaf_index) "
            "FROM audit_leaf WHERE log_id = %s",
            (log_id,),
        )
        count, lo, hi = cur.fetchone()

    print(f"transparency log {log_id}")
    print(f"  hash_algo code : {hash_algo}")
    print(f"  created (ms)   : {created_at}")
    print(f"  updated (ms)   : {updated_at}")
    print(f"  recorded leaves: {count}  (leaf_index {lo}..{hi})")

    gapless = count == 0 or (lo == 0 and hi == count - 1)
    print(f"  contiguous     : {'yes' if gapless else 'NO — gap/reorder detected!'}")

    if count:
        with conn.cursor() as cur:
            cur.execute(
                "SELECT leaf_index, leaf_hash FROM audit_leaf "
                "WHERE log_id = %s ORDER BY leaf_index LIMIT 10",
                (log_id,),
            )
            print("  first leaves   :")
            for index, leaf_hash in cur.fetchall():
                digest = bytes(leaf_hash).hex()
                print(f"    #{index:<4} {digest[:32]}...")
    print()


def main() -> None:
    parser = argparse.ArgumentParser(description="Inspect Agate's transparency log.")
    parser.add_argument(
        "--database-url",
        default=DEFAULT_DATABASE_URL,
        help=f"PostgreSQL URL (default {DEFAULT_DATABASE_URL})",
    )
    args = parser.parse_args()
    raise SystemExit(summarize(args.database_url))


if __name__ == "__main__":
    main()
