# Design: the audit append is O(n²) — bottleneck & fix

> Status: **finding (accepted), fix proposed**. Found during a load/bottleneck
> investigation of the audit write path — the system-wide throughput ceiling.

## The bottleneck

Every inspected event becomes one `AppendRecord` on the transparency log, drained
**serially** by the single `AuditOutbox` task. So the per-append cost is the
audit throughput ceiling. That per-append cost grows **linearly with the log's
current size**, making the whole write path **O(n²)**.

### Evidence (measurement)

`crates/agate-audit/tests/integration/append_scaling.rs` (ignored; run with
`--ignored --nocapture`) appends 500 records to one log over a real Postgres and
times the first vs the last 50:

| appends | avg latency |
|---|---|
| first 50 (log near-empty) | **6.9 ms** |
| last 50 (log ≈ 500 leaves) | **68.6 ms** |

**~10× slower at 500 leaves**, growing linearly. Extrapolated, a log of 1M
entries would take *seconds* per append — audit throughput collapses to ~0 as the
log grows, exactly the regime a long-lived transparency log lives in.

### Root cause

Each `AppendRecord` does a load → append-one → save cycle, and **both** the load
and the save touch *every* leaf:

- **`load`** (`command_gateway.rs`): `SELECT leaf_hash FROM audit_leaf WHERE
  log_id = $1 ORDER BY leaf_index` reads **all** leaves and reconstitutes the
  full in-memory Merkle tree — to append one leaf.
- **`save`** (`command_gateway.rs`): loops over `log.leaf_hashes()` (all leaves)
  issuing one `INSERT … ON CONFLICT DO NOTHING` each — `n` round-trips per append,
  of which `n-1` are no-ops.

So append #k costs `O(k)` on both legs → `O(n²)` for `n` appends.

## The fix (incremental / frontier Merkle)

An append-only (RFC 6962-style) Merkle log never needs all leaves in memory to
append: it needs only the **frontier** — the `O(log n)` right-edge subtree hashes
— plus the tree size. The leaves already live durably in `audit_leaf`; the
aggregate should stop hoarding them.

- **Write path** — `load` reconstitutes from the **frontier + size** (`O(log n)`),
  not all leaves; `append` folds one leaf into the frontier; `save` inserts only
  the **new** leaf (and persists the updated frontier/head). Append becomes
  `O(log n)`, flat in `n`.
- **Read path** — inclusion/consistency proofs read the specific leaves/nodes they
  need from `audit_leaf` on demand (the query gateway already owns the read side),
  rather than relying on a fully-materialised in-memory tree.

This separates the cheap, hot write path from the occasional proof reads, and is
the change that turns the measurement above flat (the harness becomes a perf
regression guard: assert `last ≈ first`).

### Scope

A focused but real refactor of the `TransparencyLog` aggregate (frontier state
instead of `Vec<leaf>`), the Merkle domain (incremental root from a frontier), and
the command gateway (`load` frontier, `save` one leaf). The on-DB schema
(`audit_leaf`) is unchanged; proofs move to reading leaves on demand. Behind the
existing `LogCommandGateway` / `MerkleProofs` ports, so the proxy and the audit
use cases are untouched.

## Interim mitigations (smaller, partial)

If the full refactor is deferred, two independent partial wins:

1. **Save only the new leaf** — the command gateway knows the append added one
   leaf; inserting just it (not re-inserting all) removes the `save`-side `O(n²)`.
   Halves the problem on its own.
2. **Batch the insert** — a single multi-row `INSERT` instead of a per-leaf loop
   removes the round-trip cost but not the `O(n)` work.

Neither removes the `load`-all-leaves `O(n²)`, so the frontier refactor is the
real fix; these only buy headroom.
