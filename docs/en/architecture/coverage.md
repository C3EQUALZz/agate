# Coverage & limitations

This page states **what Agate inspects today** versus what it forwards
uninspected, so you can deploy it without over-trusting it. The
[Threat Model](threat-model.md) describes the *target* security model; this page
is the honest *current* state. The full analysis and the roadmap to close the
gaps live in the repository at
[`docs/design/security-coverage-roadmap.md`](https://github.com/C3EQUALZz/agate/blob/main/docs/design/security-coverage-roadmap.md).

!!! warning "Read this before relying on Agate as your only guardrail"
    Agate currently enforces tool-call authorization (by **name** and by
    **argument** deny rules), secret redaction across emitted text, tool
    results, and state, and fail-closed handling of malformed events. Some
    threats named in the threat model are not yet enforced (below) — notably
    RFC 6902 JSON-Patch validation. Treat Agate as one layer, not a complete
    agent firewall, until the remaining roadmap items land.

## What is inspected

| Event / input | What happens | Policy can act? |
|---|---|---|
| `TOOL_CALL_*` (assembled) | buffered into a complete tool call, then judged; arguments screened for SSRF URLs | **Yes** — allow/deny by tool **name** and by **argument** deny rules (`[[policy.tools.deny_arguments]]`); an SSRF URL in the arguments drops the call |
| `TEXT_MESSAGE_CONTENT` | scanned for secret patterns and SSRF URLs | **Yes** — redact (literal or regex); an SSRF URL drops the chunk |
| `TOOL_CALL_RESULT` | tool-result content scanned for secret patterns and SSRF URLs | **Yes** — redact (literal or regex; the indirect-injection / exfiltration surface); an SSRF URL drops the result |
| `STATE_SNAPSHOT` / `STATE_DELTA` | size/op-count budget **and** payload scanned for secret markers | **Yes** — denied if a marker is found (a structured payload can't be masked, so it's blocked, not leaked) |
| Lifecycle (`RUN_*`, `STEP_*`) | ordering enforced by the `Run` state machine | structural only (no policy verdict) |
| Request leg (`RunAgentInput`) | `tools[*].name` authorized; `user` **and `system`** message text, plus the `context` / `forwardedProps` / inbound `state` JSON, screened for secret markers and SSRF URLs (domain hosts **resolved** and re-checked, closing DNS-rebinding) | **Yes** — reject before forwarding |
| Malformed **known** events | a recognized type with a missing/blank required field cannot be inspected → handled per `[policy].on_malformed_event` (default `terminate`) | **Yes** — fails closed by default |

## What is forwarded uninspected

| Event / input | Why it matters |
|---|---|
| **State** RFC 6902 patch operations | the payload is scanned for secret markers, but the JSON Patch operations themselves are not validated/bounded for poisoning |
| `RAW`, `CUSTOM`, `REASONING_ENCRYPTED_VALUE` | opaque — forwarded as-is, never inspected |
| Unknown / future AG-UI event types | forwarded raw |
| Hidden request fields (`system`, `forwardedProps`, `context`, inbound `state`) | not extracted, so injection into them is not screened |

## Operational limits

- **Authentication** is off by default (open proxy). Set `[proxy].api_keys`, or
  front Agate with a gateway. See [Configuration](../getting-started/configuration.md).
- **DoS budgets** today: a global concurrency cap, a request body-size limit,
  connect/read timeouts, a **per-run response budget** (`max_response_events`
  / `max_response_bytes`) that cuts off a runaway agent, and an optional
  **per-client-IP request-rate limit** (`rate_limit_per_second` /
  `rate_limit_burst`, off by default) that sheds floods with `429`. The IP is
  the connection peer, so it is only meaningful where Agate sees the real
  client.
- **Audit completeness:** records are queued to a bounded outbox; under
  sustained backpressure a record can be dropped (logged + counted) without
  stalling the data plane — so a saturated outbox can leave a gap in the
  transparency log. Monitor the audit drop metric.
- **TLS** is terminated at the proxy (required to inspect plaintext); it is off
  by default and configured under `[tls]`.

## Roadmap

Closing the remaining gaps is sequenced in
[`security-coverage-roadmap.md`](https://github.com/C3EQUALZz/agate/blob/main/docs/design/security-coverage-roadmap.md):
RFC 6902 JSON-Patch validation. (Malformed-event fail-closed, tool-argument
deny rules, secret redaction of tool results and state, per-run response
budgets, and SSRF screening on both legs — with DNS-rebinding resolution — are
now implemented.)
