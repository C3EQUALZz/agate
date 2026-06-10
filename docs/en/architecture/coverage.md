# Coverage & limitations

This page states **what Agate inspects today** versus what it forwards
uninspected, so you can deploy it without over-trusting it. The
[Threat Model](threat-model.md) describes the *target* security model; this page
is the honest *current* state. The full analysis and the roadmap to close the
gaps live in the repository at
[`docs/design/security-coverage-roadmap.md`](https://github.com/C3EQUALZz/agate/blob/main/docs/design/security-coverage-roadmap.md).

!!! warning "Read this before relying on Agate as your only guardrail"
    Agate currently enforces **two** controls end to end: tool-call
    authorization **by tool name**, and **literal text redaction**. Several
    threats named in the threat model are not yet enforced (below). Treat Agate
    as one layer, not a complete agent firewall, until the roadmap items land.

## What is inspected

| Event / input | What happens | Policy can act? |
|---|---|---|
| `TOOL_CALL_*` (assembled) | buffered into a complete tool call, then judged | **Yes** — allow/deny **by tool name** |
| `TEXT_MESSAGE_CONTENT` | scanned for secret patterns | **Yes** — redact (literal, case-insensitive substring) |
| Lifecycle (`RUN_*`, `STEP_*`) | ordering enforced by the `Run` state machine | structural only (no policy verdict) |
| `STATE_SNAPSHOT` / `STATE_DELTA` | size / op-count budget checked | **No** — content auto-allowed |
| Request leg (`RunAgentInput`) | `tools[*].name` authorized; `user` message text screened for SSRF URLs | **Yes** — reject before forwarding |
| Malformed **known** events | a recognized type with a missing/blank required field cannot be inspected → handled per `[policy].on_malformed_event` (default `terminate`) | **Yes** — fails closed by default |

## What is forwarded uninspected

| Event / input | Why it matters |
|---|---|
| Tool-call **arguments** | passed to the policy but only the **name** is checked — argument payloads (injection, SSRF, oversized data) are not yet policy-screened |
| Tool **results** (`TOOL_CALL_RESULT`) | auto-allowed — the indirect-injection and exfiltration surface is not inspected |
| **State** mutation content | only byte-size/op-count budgets; RFC 6902 patch operations are not validated |
| `RAW`, `CUSTOM`, `REASONING_ENCRYPTED_VALUE` | opaque — forwarded as-is, never inspected |
| Unknown / future AG-UI event types | forwarded raw |
| Hidden request fields (`system`, `forwardedProps`, `context`, inbound `state`) | not extracted, so injection into them is not screened |

## Operational limits

- **Authentication** is off by default (open proxy). Set `[proxy].api_keys`, or
  front Agate with a gateway. See [Configuration](../getting-started/configuration.md).
- **DoS budgets** today: a global concurrency cap, a request body-size limit,
  and connect/read timeouts. There is **no** per-key/per-session rate limit and
  **no** size/rate cap on the streamed response to the client.
- **Audit completeness:** records are queued to a bounded outbox; under
  sustained backpressure a record can be dropped (logged + counted) without
  stalling the data plane — so a saturated outbox can leave a gap in the
  transparency log. Monitor the audit drop metric.
- **TLS** is terminated at the proxy (required to inspect plaintext); it is off
  by default and configured under `[tls]`.

## Roadmap

Closing the remaining gaps is sequenced in
[`security-coverage-roadmap.md`](https://github.com/C3EQUALZz/agate/blob/main/docs/design/security-coverage-roadmap.md):
tool-argument inspection, bringing state and tool-results under the policy,
rate/output budgets, and a richer (regex/glob + argument-condition) TOML policy
language. (Fail-closed handling of malformed known events is now implemented —
see `[policy].on_malformed_event`.)
