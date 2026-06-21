# Design: the audit append is O(n²) — bottleneck & fix

> Status: **fixed.** Found during a load/bottleneck investigation of the audit
> write path (the system-wide throughput ceiling); fixed by making the append a
> single-leaf `INSERT`. The measurement below is now a perf regression guard
> (`append_scaling.rs`, no longer ignored).
>
> **Result:** append latency went from ~10× growth at 500 leaves to **flat** —
> first-50 vs last-50 average `6.9ms → 68.6ms` (before) became `1.7ms → 0.9ms`
> (after, at 300 leaves; ~0.5×). Append is now O(1) in the log's size.

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

## The fix (direct single-leaf append)

The investigation surfaced the decisive fact: **`append` never computes the root.**
The aggregate's `append()` only assigns an index and pushes one leaf hash; the root
(and proofs) are recomputed on demand by the checkpoint/query paths. So the hot
write path needs neither all the leaves nor a frontier — it needs only to assign
the next index and persist one new leaf.

A new `append_record` on the `LogCommandGateway` port does exactly that, in `O(1)`:

- `SELECT 1 FROM audit_log` to confirm the log exists (else `None` → `LogNotFound`).
- `SELECT COALESCE(MAX(leaf_index) + 1, 0)` for the next index. The log is
  **append-only and single-writer** (the audit outbox drains serially), so this
  read-then-insert is race-free.
- Hash the leaf with the same `MerkleHasher` the aggregate uses, then a plain
  `INSERT` of that one row. Plain on purpose: the `(log_id, leaf_index)` unique
  constraint must *reject* a duplicate index loudly, not swallow it — under
  single-writer append-only it never conflicts, and if it ever did (a second
  writer, a replay) we want a storage error, not a lost leaf reported as success.

`AppendRecordHandler` now calls `append_record` instead of load → `append` → save.
The aggregate, the Merkle domain, the `audit_leaf` schema, and the read/proof paths
are all unchanged — proofs and checkpoints still recompute the root from the stored
leaves, which are byte-identical to before. Both `O(n)` legs (load-all, re-insert-all)
are gone.

This is simpler and safer than the frontier refactor first sketched here: no aggregate
state change, no incremental-root code, nothing for proofs to depend on. The frontier
would have been necessary only if append had to produce a root cheaply — it doesn't.

### Verification

`append_scaling.rs` (no longer `#[ignore]`) is now a regression guard: it appends
300 records and asserts the last-50 average latency stays within `4×` of the
first-50. Measured after the fix: `1.7ms → 0.9ms` (≈`0.5×`), versus the pre-fix
`6.9ms → 68.6ms` (`10×`). The integration `dispatcher` test appends via this path
then verifies an inclusion proof, confirming the leaf hashes/indices match what the
aggregate produced.

## Alternatives considered (rejected)

- **Frontier / incremental Merkle** — carry the `O(log n)` right-edge in the
  aggregate so `load`/`save` avoid all leaves. Real work (aggregate state change,
  incremental-root code, proof reads on demand) for no gain over direct append,
  since append never needs a root. Rejected.
- **Save only the new leaf / batch the insert** — partial wins that remove the
  `save`-side cost but not the `load`-all-leaves leg. Subsumed by the direct
  append, which removes both legs.
