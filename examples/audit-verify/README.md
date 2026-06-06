# audit-verify

Inspect Agate's **transparency log**. Agate records every inspected
`(event, verdict)` decision as a leaf in an append-only, RFC 6962 Merkle log
(Postgres-backed). This tool reads that log to show the decisions from the
[`protected-demo`](../protected-demo) were durably recorded — the "you can prove
what happened" half of Agate, alongside deny and redact.

## What it reads

Two tables (`crates/agate-audit/migrations/0001_init.sql`):

| Table | Meaning |
| --- | --- |
| `audit_log` | One row per log: `id` (UUID), `created_at`/`updated_at` (Unix ms), `hash_algo` (epoch hash-algorithm code). |
| `audit_leaf` | One row per recorded decision: `log_id`, `leaf_index` (0-based, monotonic), `leaf_hash` (bytes hashed into the tree). |

The **gapless `leaf_index` sequence** is the tamper-evidence: removing or
reordering a decision breaks the Merkle head. `contiguous: yes` means no gaps.

## Run

The demo's Postgres is not published to the host by default. Either add
`ports: ["5432:5432"]` to the `db` service in
[`../protected-demo/docker-compose.yaml`](../protected-demo/docker-compose.yaml),
then:

```bash
# 1) start the demo and send at least one run through Agate first:
#    (in ../protected-demo)  docker compose up --build
#    (in ../protected-demo)  uv run protected-demo-client

# 2) inspect the log:
uv sync
uv run audit-verify
```

Or point it at any reachable Postgres:

```bash
uv run audit-verify --database-url postgres://agate:agate@localhost:5432/agate
```

## Example output

```
transparency log 3f6c1e2a-....
  hash_algo code : 1
  created (ms)   : 1733443200000
  updated (ms)   : 1733443205000
  recorded leaves: 9  (leaf_index 0..8)
  contiguous     : yes
  first leaves   :
    #0    9f2b1c...
    #1    a17e44...
```

Each leaf corresponds to one inspected AG-UI event and its verdict (allow / deny
/ redact).

## Layout

```
src/audit_verify/
├── config.py                 # Config (database url / timeout / sample size)
├── domain/log.py             # TransparencyLogSummary (+ is_contiguous), LeafSample
├── gateways/audit_log.py     # AuditLogReader port + PostgresAuditLogReader adapter
└── cli.py                    # argument parsing + rendering
```

`uv run pytest` runs the `is_contiguous` / digest unit tests (no Postgres needed).

> **VERIFY:** this reads the schema directly because the Agate build targeted
> here exposes the log via Postgres, not (yet) via an HTTP inclusion-proof
> endpoint. The leaf stores a **hash**, not the decoded event, so you see
> digests, not payloads. If your Agate build ships an inclusion-proof API,
> prefer it.
