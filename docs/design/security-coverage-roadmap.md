# Design: security coverage — current state & roadmap

> Status: **accepted (living document)**. This record reconciles the
> [threat model](agate-proxy-threat-model.md) (the *target* security model) with
> what the code enforces *today*, and sequences the work to close the gap. The
> threat model says what Agate intends to defend; this document says how far the
> implementation has got and what comes next.

## Why this document exists

The threat model enumerates ~10 classes of attack across six assets. The data
plane currently enforces a subset: **tool-call authorization by name** and
**text redaction by literal substring**. Everything else is either
`auto-allow`d at the policy seam or forwarded as an uninspected raw frame. That
is the intended MVP shape — the proxy shipped an allow-all policy behind a
`PolicyPort` seam first, then grew real policy — but as a product it currently
*promises* (via the threat model) more than it *enforces*. This document makes
that gap explicit so operators do not over-trust the proxy, and so the
remaining work has an agreed order.

The companion published page,
[Coverage & limitations](../en/architecture/coverage.md), is the
operator-facing summary of the table below.

## 1. Coverage today (claim → reality)

Each row maps a threat-model asset/threat to the code that does (or does not)
enforce it. File references are to `crates/` on `main`.

| Asset / threat | Threat model promises | Enforced today | Gap |
|---|---|---|---|
| **A1 — tool authorization** | verdict on tool **and arguments** | name authorized by `ToolAuthorizer` (exact/glob/regex); arguments checked by `ArgumentInspector` against `[[policy.tools.deny_arguments]]` literal/regex markers, optionally scoped to one parsed field by `path` (`agate-policy/.../argument_inspector.rs`) | text matching on a field or the raw blob — value-aware predicates (e.g. URL resolves to a private IP) still to come (P3 SSRF) |
| **A2 — sensitive-data exfiltration** | redact text, screen URLs | literal-or-regex redaction across `TEXT_MESSAGE_CONTENT` **and** tool results; a secret in a state payload is denied (can't be masked); SSRF screen on **both legs** (request `user` messages and response-leg tool-call arguments, messages, and tool results) that **resolves** domain hosts and re-checks the addresses (closes DNS-rebinding) | — |
| **A3 — instruction integrity / prompt injection** | resist injection incl. indirect (URL content, tool results) | tool **results** now reach the policy and are secret-redacted; broader injection heuristics still absent | partial — no anti-injection heuristics yet |
| **A4 — shared-state integrity** | verdict on state-mutating events; validate & bound JSON Patch | `STATE_*` payload reaches the policy (a secret marker is denied); a `STATE_DELTA` is validated as well-formed RFC 6902 (known op kinds, present path) and bounded by op count, pointer depth, and per-op value size | done at the structural level — semantic path-allowlisting (which paths an op may touch) is future |
| **A5 — availability / DoS** | size/time **and rate** budgets per run and per connection | global concurrency cap, body-size limit, connect/read timeouts, **per-run response budget** (`max_response_events`/`max_response_bytes`), **per-client-IP rate limit** (`rate_limit_per_second`/`rate_limit_burst`, `429`) | done for the request leg; per-session (sub-IP) limits still open |
| **A6 — audit tamper-evidence** | every inspected event + verdict recorded | recorded via a bounded outbox; fill exported as `agate_audit_outbox_depth`/`_capacity`; on a full outbox the `[audit].outbox_on_full` policy applies — `block` (backpressure, never drop) or `shed` (drop, loudly logged + counted) | a drop is no longer silent (gauge + counter + log); periodic signed checkpoints (PR #72) bound how much an undetected gap could hide |

### Cross-cutting gaps (not a single asset)

- **G1 — fail-open on malformed *known* events.** ✅ **Closed.**
  `inspect_stream` now distinguishes a recognized-but-malformed event
  (`AgUiError::is_malformed_known()` — a known `type` with a missing/blank
  required field) from an uninspectable frame, and applies
  `[policy].on_malformed_event` (`forward` | `drop` | `terminate`), defaulting
  to the secure `terminate`. An unknown type / non-object / non-JSON frame still
  forwards unchanged.
- **G2 — no cross-run / per-session state.** ✅ **Closed (replay memory).** An
  optional per-session ledger (`[policy.session_memory]`, off by default)
  quarantines a tool by name once it is denied, so the agent cannot retry it —
  with varied arguments — in a later run of the same session. It only *adds* a
  denial over the stateless policy (a backend outage degrades to "no memory",
  never to a wrong allow). The session is keyed on the AG-UI **`threadId`**: the
  proxy derives the `SessionId` deterministically from it (and the `RunId` from
  `runId`), so a returning conversation maps to the same session across runs —
  without that, every request would be its own session and the ledger could
  never fire. Two backends (`[policy.session_memory].backend`): a process-local
  sliding-TTL ledger (default), or **Redis** (shared across replicas and
  restarts), the latter fail-open — an unreachable Redis degrades to no memory,
  never a wrong allow. Broader per-key quotas remain future work.
- **G3 — hidden request fields uninspected.** ✅ **Closed.** The request leg
  now also screens `system` message content and the `context`,
  `forwardedProps`, and inbound `state` JSON (`RequestContent.hidden_fields`),
  applying the same secret-marker + SSRF screen as `user` messages.
- **G4 — opaque & unknown events are pass-through only.** `RAW`, `CUSTOM`,
  `REASONING_ENCRYPTED_VALUE`, and any unknown/future AG-UI type forward raw.
  The threat model says "pass-through **or drop**"; there is no drop option in
  config.

## 2. Roadmap

Ordered by impact-per-effort. Each item is a self-contained change behind the
existing `PolicyPort` / `InspectedAction` / `Verdict` seam — none require a new
architecture, which is the payoff of the hexagonal layering.

### Phase 1 — close the highest-impact gaps

1. **Tool-argument inspection (A1).** ✅ **Done (literal).** `ArgumentInspector`
   denies a permitted tool call whose arguments contain a configured marker;
   rules are `[[policy.tools.deny_arguments]]` (`{ tool?, contains }`,
   case-insensitive), optionally scoped to one tool, evaluated after name
   authorization. Structured/JSONPath predicates over the parsed arguments
   remain for P2 (the `ArgumentRule` value object is the seam to grow them
   behind).
2. **Fail-closed on malformed known events (G1).** ✅ **Done.** `inspect_stream`
   distinguishes "unrecognized type → forward" (`Ok(None)`) from "recognized but
   malformed → `Err`" (`AgUiError::is_malformed_known`). The behavior is the
   `[policy].on_malformed_event` knob (`forward` | `drop` | `terminate`),
   defaulting to `terminate` (matching the structural-reject posture of the
   `Run` state machine).
3. **State & tool-result inspection (A3/A4).** ✅ **Done (secret level).**
   `InspectedAction` gained `ToolResult` and `StateMutation` variants, so both
   reach the policy instead of `auto-allow`. Tool-result content is
   secret-redacted in place (`Verdict::Transform`); a state payload cannot be
   masked, so a secret marker found in it is **denied** rather than leaked. ✅ A
   `STATE_DELTA` is now also validated as a well-formed RFC 6902 patch (known op
   kinds, present path) and bounded by op count, pointer depth, and per-op value
   size (the adapter measures; the domain budgets). Still to come: richer
   anti-injection heuristics on tool results, and semantic path-allowlisting.
4. **Rate & output budgets (A5).** ✅ **Done.** A per-run `ResponseBudget`
   (`max_response_events` / `max_response_bytes`, `0` = unlimited) caps the SSE
   leg: crossing it ends the run with a `RUN_ERROR`, so a runaway/hostile agent
   cannot stream unbounded output to the client. The **request leg** is now
   rate-limited per client IP by a `governor`-backed middleware
   (`rate_limit_per_second` / `rate_limit_burst`, `0` = disabled): a source IP
   over budget is shed with `429 Too Many Requests` + `Retry-After`, and the
   keyed map is pruned on a timer so distinct IPs cannot grow it without bound.
   The IP is the connection peer, so it is meaningful only where Agate sees the
   real client. **Still open:** sub-IP **per-session / per-API-key** limits, and
   honoring a trusted `X-Forwarded-For` when fronted by a known balancer.

### Phase 2 — richer policy authoring (static TOML first)

Chosen direction: **extend the static TOML policy language** before any plugin
engine — it closes most real cases without new infrastructure or a sandbox.

- **Patterns:** ✅ a shared `Pattern` value object (literal | regex) backs both
  secret redaction (`[policy].redact` + `[policy].redact_regex`) and argument
  deny rules (`contains` | `matches`); literal stays the default. ✅ **Tool
  names** now match by `ToolMatcher` (exact | glob | regex), selected per entry
  in `[policy.tools].names` by a `glob:` / `regex:` prefix (bare = exact);
  matching is anchored to the whole name and case-sensitive, so `search` never
  matches `research`.
- **Argument predicates:** ✅ literal and regex markers
  (`[[policy.tools.deny_arguments]]` `contains` / `matches`), and ✅ a `path`
  scope (a dotted path like `url` / `config.endpoint`) that matches the marker
  against one field of the *parsed* arguments rather than the whole blob — so
  `{ tool = "fetch", path = "url", matches = "169\.254" }` screens `args.url`
  without firing on an unrelated field. Still to come: predicates that go beyond
  text matching on a field (e.g. "deny when `args.url` *resolves* to a private
  IP"), behind the same `ArgumentRule` seam — see SSRF hardening in Phase 3.
- **Result & state rules:** ✅ tool results are secret-redacted and now also
  screened by `[[policy.tools.deny_results]]` deny rules (a forbidden result is
  blocked before the client, optionally scoped by `tool`/`path`); state
  mutations carrying a secret are denied (they cannot be masked in place). ✅ A
  `STATE_DELTA` is validated as a well-formed RFC 6902 patch and bounded (op
  count, pointer depth, per-op value size).
- **Per-tool policies:** ✅ argument and result deny rules are tool-scoped (an
  optional `tool` on each rule; result rules correlate the tool name from the
  call's start). The redaction secret list stays global by design (content-based,
  not tool-based).
- **Hot-reload:** re-read the ruleset on `SIGHUP` / file-watch so operators
  iterate without a restart (the ruleset is already immutable and built at the
  composition root — swap the `Arc` behind the `PolicyPort`). **Deferred** as an
  ops convenience, not a coverage gap.

A **plugin engine** — rules as code rather than fixed TOML knobs — is the
"product" step beyond the static language. ✅ A **CEL backend** is the first one,
landed behind the `PolicyPort` seam exactly as planned: built only with the
`policy-cel` Cargo feature and selected with `[policy].backend = "cel"`, it
evaluates operator-authored [CEL](https://cel.dev/) rules (a TOML list of
`[[rule]]` with a `when` boolean expression and a `deny`/`redact`/`allow`
effect), compiled at startup, first-match-wins. CEL's non-Turing-completeness
keeps every decision terminating, and the rules see the same event projection
the static engine inspects (`action.{kind,name,arguments_json,content_json,
state_json,…}` plus the run `context`). It reuses the shared decision→verdict
`lift` so the redaction invariants cannot drift from the static adapter. The
proxy was untouched — proof the seam holds. ✅ Hot-reload landed too: the rule set
sits behind an `ArcSwap` swapped on `SIGHUP` and (opt-in) on file-change, always
fail-safe (a bad/empty reload keeps the running policy).

✅ A **Rego (OPA) backend** is the second engine, on the same seam: built with the
`policy-rego` feature, selected with `[policy].backend = "rego"`, it evaluates an
operator's Rego policy (package `agate.policy`, rule `decision`) through the
pure-Rust [`regorus`](https://github.com/microsoft/regorus) interpreter — no
sidecar, no WASM runtime. It shares the *same* `event_view` projection and
decision→verdict `lift` as CEL (so the two engines see identical input and cannot
drift), reuses the `SIGHUP`/file-watch reload via a shared `ReloadablePolicy`
trait, and fails closed on an evaluation error or a malformed `decision`. Each
decision clones the prepared engine (~tens of µs) so concurrent decisions never
contend. A **WASM** backend (wasmtime/extism, for arbitrary-language plugins
behind a sandbox) and a policy marketplace can land the same way later.

### Phase 3 — defense-in-depth

- **SSRF hardening (A2):** ✅ **done.** URLs are screened on both legs — request
  `user` messages and response-leg tool-call arguments, message chunks, and tool
  results — and a domain host is resolved through the `HostResolver` port with
  its addresses re-checked (DNS-rebinding closed). URL extraction splits on JSON
  punctuation as well as whitespace, so a URL embedded in a JSON value — the usual
  shape of tool-call arguments and tool results, e.g.
  `{"url":"http://169.254.169.254/"}` — is isolated and screened, not only URLs
  sitting alone in prose. Response-leg screening is per-event (a URL split across
  streamed chunks is not reassembled).
- **Audit completeness (A6):** ✅ done — the outbox fill is exported
  (`agate_audit_outbox_depth`/`_capacity`) and `[audit].outbox_on_full` chooses
  `block` (backpressure the data plane) or `shed` (drop with a loud
  log + drop counter), so a gap in the tamper-evident log is never silent.
- **Per-session memory (G2):** ✅ done — `[policy.session_memory]` (off by
  default) keeps a per-session ledger so a tool denied in one run is refused
  (by name) for the rest of the session. A new `SessionMemory` port with a
  process-local sliding-TTL adapter; a shared (Redis) backend for multi-replica
  deployments is the next step.
- **Hidden-field screening (G3):** ✅ done — `system` content and the
  `context` / `forwardedProps` / inbound `state` JSON are extracted into
  `RequestContent.hidden_fields` and screened on the request leg.

## 3. Non-goals (unchanged from the threat model)

- Transport MitM (handled by TLS at the proxy).
- The agent's own internal logic and the LLM provider's safety.
- Operator-side log tampering (mitigated by `agate-audit`'s transparency log).

## 4. Tracking

This document is the source of truth for coverage status. When a roadmap item
lands, move its row from "Gap" to enforced in §1 and update the published
[Coverage & limitations](../en/architecture/coverage.md) page in the
same PR (EN + RU).
