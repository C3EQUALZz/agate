# Policy evaluation is synchronous and inline, bounded by a rule cap (not the timeout)

The CEL and Rego policy backends evaluate each event **synchronously**, inline on
the calling tokio worker, with no `.await` inside `PolicyPort::decide`. We
deliberately do **not** offload evaluation to `tokio::task::spawn_blocking`,
even though `decide` is `async`.

## Context

`decide` is called on the hot path for **every** streamed event. CEL and Rego
are both non-Turing-complete (no loops or recursion), so a single evaluation is
microsecond-scale and always terminates. The `FailModePolicy` decorator wraps
each decision in `tokio::time::timeout`, which led to a natural-looking but
**false** belief — stated in the old module docs — that the timeout reliably
bounds any decision. It does not: a future that never yields cannot be
preempted, so `timeout` only fires for policies that actually `.await` (e.g. an
engine consulting an external service), never mid-evaluation of a synchronous
interpreter.

## Decision

Keep evaluation synchronous and inline. Guarantee bounded cost **structurally**
instead of relying on the timeout:

- **CEL** — cap the number of `[[rule]]` entries (`policy.cel.max_rules`,
  default 1000). Cost is linear in the rule count; the cap bounds it. A policy
  over the cap is rejected at load and at reload (the running policy is kept).
- **Rego** — a policy is a single `decision` rule, so the rule-count cap does not
  map; termination relies on Rego's non-Turing-completeness, and cost is bounded
  by the operator's policy file. A dedicated bound is left as future work.

## Consequences

`spawn_blocking` was rejected because a per-event thread hand-off would regress
the microsecond hot path far more than it would help — operator rule sets are
trusted and bounded, so the worker-stall risk it addresses is not realistic. The
trap this avoids: a future reader sees a synchronous body in an `async fn` and
"fixes" it with `spawn_blocking`. This ADR records that the synchronous shape is
deliberate. If a backend ever gains genuinely expensive or untrusted evaluation,
revisit this — offload that backend specifically and make its `decide` actually
yield so the `FailModePolicy` timeout can take effect.
