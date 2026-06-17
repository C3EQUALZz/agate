# Design: concurrency & high-load audit

> Status: **accepted**. A read-through audit of the data plane's
> concurrency-bearing hot paths plus a reproducible stress test, run once the
> security-coverage roadmap was closed. The question it answers: *is the proxy
> correct under high parallel load — no data races, deadlocks, lost audit
> records, or task leaks?*

## Scope

The data plane sees one request per proxied agent run, each fanning out to
streaming, the policy seam, and the audit outbox. The audit covered every shared
mutable structure and background task on that path:

1. **Audit outbox** — `agate-server/.../audit/{sink,outbox,appender}.rs`. A
   bounded `tokio::mpsc`: many request tasks produce, one `AuditOutbox` task
   drains. `FullPolicy { Block | Shed }`.
2. **Session-memory ledger** — `agate-proxy/.../policy/session_memory.rs`. An
   `Arc<Mutex<HashMap<…>>>` with a sliding TTL and a background pruner.
3. **Rate limiter** — `agate-proxy/.../middlewares/rate_limit.rs`. A `governor`
   keyed limiter with a `Weak`-held pruner.
4. **Concurrency cap + graceful shutdown** — the tower concurrency limit, the
   `axum-server` graceful `Handle`, the outbox `JoinHandle` drain, the
   checkpoint task abort.
5. **FailModePolicy** — the `tokio::time::timeout` around a policy decision.
6. **Checkpoint scheduler** — the periodic STH timer loop.

## Verdict

**The data plane is sound for correctness under high load.** No data races (the
`std::sync::Mutex` in the session ledger is released before every `.await`), no
deadlocks, no audit record lost without a counter increment (a single-consumer
FIFO outbox, with loud counted drops on both the `Shed` and closed-channel
paths), and no task leaks (both pruners are `Weak`-gated and end when their owner
is dropped). This is now backed by a stress test (below), not just inspection.

The residual findings are about **shutdown quality / availability**, not
correctness:

| Severity | Finding | Disposition |
|---|---|---|
| Med | `main` calls `handle.graceful_shutdown(None)` — no deadline, so one hung upstream stalls process exit. | Mitigated in practice by the upstream **read timeout** (a stalled stream ends within `read_timeout`, default 1 min). A configurable shutdown grace deadline is a possible follow-up. |
| Med | The checkpoint task is stopped with `.abort()`, which can land mid-`dispatch` and abandon an open audit scope/transaction (it rolls back, so no data loss, but it is an abrupt cancel holding a DB connection). | Follow-up: stop the loop **cooperatively** at a tick boundary (a `Notify` / cancellation token) instead of aborting. |
| Low | The outbox depth gauge `capacity - tx.capacity()` is racy under concurrent producers. | Accepted: it is observation-only and never drives the shed decision (that uses `try_send`'s own atomic result). Documented in the A6 work. |

## Stress test

`crates/agate-server/tests/e2e/stress.rs` boots the full stack (proxy + a stub
AG-UI agent + a real PostgreSQL audit store via testcontainers) and fires **64
concurrent proxied runs**, each producing three inspected events. It asserts:

- every run completes and is forwarded whole — no deadlock, no truncation, no
  leaked concurrency permit;
- the last of the `64 × 3` records is present in the transparency log — so under
  concurrent outbox producers the single consumer drained every record in order
  with none lost.

The run count is held under the default concurrency cap (256) so none is shed.
This is the empirical counterpart to the inspection-level verdict above; it runs
in the ubuntu-only e2e suite (where Docker is available).
