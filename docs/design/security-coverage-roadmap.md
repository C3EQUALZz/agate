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
| **A1 — tool authorization** | verdict on tool **and arguments** | name authorized by `ToolAuthorizer`; arguments checked by `ArgumentInspector` against `[[policy.tools.deny_arguments]]` literal markers (`agate-policy/.../argument_inspector.rs`) | literal-only — structured/JSONPath predicates still to come (P2) |
| **A2 — sensitive-data exfiltration** | redact text, screen URLs | literal, case-insensitive substring redaction on `TEXT_MESSAGE_CONTENT` only (`text_redactor.rs`); request-leg SSRF screen on `user` messages, no DNS resolution | tool args / tool results / state not redacted; SSRF is best-effort and request-leg only |
| **A3 — instruction integrity / prompt injection** | resist injection incl. indirect (URL content, tool results) | tool **results** now reach the policy and are secret-redacted; broader injection heuristics still absent | partial — no anti-injection heuristics yet |
| **A4 — shared-state integrity** | verdict on state-mutating events; validate & bound JSON Patch | `STATE_*` payload now reaches the policy (a secret marker in it is denied) plus `byte_size`/`op_count` budgets | partial — RFC 6902 ops still unvalidated/unbounded |
| **A5 — availability / DoS** | size/time **and rate** budgets per run and per connection | global concurrency cap, body-size limit, connect/read timeouts, **per-run response budget** (`max_response_events`/`max_response_bytes`) | partial — no per-key/session **rate** limit yet |
| **A6 — audit tamper-evidence** | every inspected event + verdict recorded | recorded via a bounded outbox channel | **silent drop under backpressure** — `record()` returns `()`, data plane never learns of a lost record |

### Cross-cutting gaps (not a single asset)

- **G1 — fail-open on malformed *known* events.** ✅ **Closed.**
  `inspect_stream` now distinguishes a recognized-but-malformed event
  (`AgUiError::is_malformed_known()` — a known `type` with a missing/blank
  required field) from an uninspectable frame, and applies
  `[policy].on_malformed_event` (`forward` | `drop` | `terminate`), defaulting
  to the secure `terminate`. An unknown type / non-object / non-JSON frame still
  forwards unchanged.
- **G2 — no cross-run / per-session state.** `SessionId` is threaded through but
  the policy is stateless: a denied action can be retried in a fresh run within
  the same session; there are no per-session or per-key quotas.
- **G3 — hidden request fields uninspected.** The request leg extracts only
  `offered_tools` and `user` message text. `system` prompt, `forwardedProps`,
  `context.value`, and inbound `state` are not inspected, so injection into
  those fields is never screened.
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
   masked, so a secret marker found in it is **denied** rather than leaked.
   Still to come: bounding/validating `STATE_DELTA` RFC 6902 operations (op
   kinds, path depth, value size) and richer anti-injection heuristics on tool
   results.
4. **Rate & output budgets (A5).** ✅ **Output done.** A per-run `ResponseBudget`
   (`max_response_events` / `max_response_bytes`, `0` = unlimited) caps the SSE
   leg: crossing it ends the run with a `RUN_ERROR`, so a runaway/hostile agent
   cannot stream unbounded output to the client. **Still to come:** per-API-key
   / per-session **rate** limiting on the request leg (requests per unit time) —
   a stateful middleware, likely on a dedicated dependency (`governor`),
   tracked separately.

### Phase 2 — richer policy authoring (static TOML first)

Chosen direction: **extend the static TOML policy language** before any plugin
engine — it closes most real cases without new infrastructure or a sandbox.

- **Patterns:** regex / glob for tool names (`ToolName`) and for secret
  redaction (`SecretPattern`), alongside the current literal match. Keep literal
  as the default for safety and speed.
- **Argument predicates:** grow `[[policy.tools.deny_arguments]]` beyond the
  literal `contains` marker — JSONPath / structured conditions over the parsed
  arguments (e.g. "deny when `args.url` resolves to a private IP"), behind the
  existing `ArgumentRule` value object.
- **Result & state rules:** redaction/deny conditions for tool results and
  state mutations.
- **Per-tool policies:** today the ruleset is flat (one `ToolPolicy` + one
  secret list); allow per-tool argument/result rules.
- **Hot-reload:** re-read the ruleset on `SIGHUP` / file-watch so operators
  iterate without a restart (the ruleset is already immutable and built at the
  composition root — swap the `Arc` behind the `PolicyPort`).

A **plugin engine** (Rego/OPA, CEL, or WASM via a sandbox) — rules as code,
hot-reload, a policy marketplace — is the eventual "product" step. It is left as
a future seam: the `PolicyPort` already isolates the data plane from how a
verdict is computed, so a `WasmPolicy` adapter can land later without touching
the proxy.

### Phase 3 — defense-in-depth

- **SSRF hardening (A2):** resolve DNS and re-check against the blocklist to
  close DNS-rebinding; extend the screen to tool arguments and response-leg
  content, not just request-leg `user` messages.
- **Audit completeness (A6):** surface outbox backpressure to operators
  (a saturation metric + a configurable policy: block the data plane, or shed
  with a loud alert) so a gap in the tamper-evident log is never silent.
- **Per-session memory (G2):** optional session-scoped state so a denied action
  cannot be replayed across runs.
- **Hidden-field screening (G3):** extract and inspect `system`,
  `forwardedProps`, `context`, and inbound `state` on the request leg.

## 3. Non-goals (unchanged from the threat model)

- Transport MitM (handled by TLS at the proxy).
- The agent's own internal logic and the LLM provider's safety.
- Operator-side log tampering (mitigated by `agate-audit`'s transparency log).

## 4. Tracking

This document is the source of truth for coverage status. When a roadmap item
lands, move its row from "Gap" to enforced in §1 and update the published
[Coverage & limitations](../en/architecture/coverage.md) page in the
same PR (EN + RU).
