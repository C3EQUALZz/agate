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
| **A1 — tool authorization** | verdict on tool **and arguments** | `ToolAuthorizer` matches the tool **name** only (`agate-policy/.../tool_authorizer.rs`); arguments are buffered, handed to the policy, and ignored | **arguments uninspected** |
| **A2 — sensitive-data exfiltration** | redact text, screen URLs | literal, case-insensitive substring redaction on `TEXT_MESSAGE_CONTENT` only (`text_redactor.rs`); request-leg SSRF screen on `user` messages, no DNS resolution | tool args / tool results / state not redacted; SSRF is best-effort and request-leg only |
| **A3 — instruction integrity / prompt injection** | resist injection incl. indirect (URL content, tool results) | not implemented; tool results are `auto-allow` | **whole asset open** |
| **A4 — shared-state integrity** | verdict on state-mutating events; validate & bound JSON Patch | `STATE_DELTA`/`STATE_SNAPSHOT` checked for `byte_size`/`op_count` budgets only; never reaches the policy (`adapter.rs` maps them to `InspectedAction::Other`) | **state content uninspected**; RFC 6902 ops unvalidated |
| **A5 — availability / DoS** | size/time **and rate** budgets per run and per connection | global concurrency cap, body-size limit, connect/read timeouts | no rate limit; no per-connection event budget; **client SSE response unbounded** |
| **A6 — audit tamper-evidence** | every inspected event + verdict recorded | recorded via a bounded outbox channel | **silent drop under backpressure** — `record()` returns `()`, data plane never learns of a lost record |

### Cross-cutting gaps (not a single asset)

- **G1 — fail-open on malformed *known* events.** `presentation/stream.rs` does
  `to_fragment(&value).ok().flatten()`: a parse error on a recognized event
  (missing/blank required field, e.g. a `TOOL_CALL_START` with a blank
  `toolCallId`) collapses to "not inspectable → forward raw", bypassing policy.
  The domain and the AG-UI mapper now *reject* a blank correlating id
  (`AgUiError::BlankField`), but the stream still swallows that rejection. For a
  security proxy a malformed known event should fail **closed** (or be
  operator-configurable), not pass through.
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

1. **Tool-argument inspection (A1).** Extend the policy language so a rule can
   match on tool **arguments**, not just the name. Concretely: add
   argument-condition rules to `ToolPolicy` — a JSONPath/`serde_json::Value`
   predicate set (e.g. "deny `shell` when `args.cmd` matches `rm -rf`", "deny
   any tool whose args contain a private-IP URL"). The arguments already reach
   the policy; this is policy-side work plus config surface.
2. **Fail-closed on malformed known events (G1).** In `inspect_stream`,
   distinguish "unrecognized type → forward" (`Ok(None)`) from "recognized but
   malformed → `Err`". Make the malformed-known-event behavior an explicit
   policy/config knob (`forward` | `drop` | `terminate`), defaulting to the
   secure choice (`terminate`, matching the structural-reject posture of the
   `Run` state machine).
3. **State & tool-result inspection (A3/A4).** Add `InspectedAction::ToolResult`
   and `InspectedAction::StateMutation` variants so the policy *sees* these
   events instead of `auto-allow`. Then: bound and validate `STATE_DELTA` RFC
   6902 operations (op kinds, path depth, value size), and allow redaction/deny
   on tool-result content (the indirect-injection / exfiltration surface).
4. **Rate & output budgets (A5).** Per-API-key and/or per-session rate limiting
   on the request leg; a per-connection event-count / response-byte budget on
   the SSE leg so a hostile agent cannot stream unbounded output to the client.

### Phase 2 — richer policy authoring (static TOML first)

Chosen direction: **extend the static TOML policy language** before any plugin
engine — it closes most real cases without new infrastructure or a sandbox.

- **Patterns:** regex / glob for tool names (`ToolName`) and for secret
  redaction (`SecretPattern`), alongside the current literal match. Keep literal
  as the default for safety and speed.
- **Argument predicates:** the JSONPath/condition rules from Phase 1, expressed
  in `[policy.tools]`.
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
