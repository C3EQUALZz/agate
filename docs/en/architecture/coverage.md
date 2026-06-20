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
    results, and state, SSRF screening on both legs, RFC 6902 patch
    validation/bounding, fail-closed handling of malformed events, and optional
    cross-run replay memory. It is still one layer, not a complete agent
    firewall (it does not model state semantics) — see the roadmap for what
    remains.

## What is inspected

| Event / input | What happens | Policy can act? |
|---|---|---|
| `TOOL_CALL_*` (assembled) | buffered **per call id** into a complete tool call, then judged; arguments screened for SSRF URLs. The held `START`/`ARGS` frames are relayed only if the call is allowed, so concurrent or never-closed (no `TOOL_CALL_END`) calls can't leak a denied call's frames; an unclosed call is still judged at run end | **Yes** — allow/deny by tool **name** and by **argument** deny rules (`[[policy.tools.deny_arguments]]`); an SSRF URL in the arguments drops the call |
| `TEXT_MESSAGE_CONTENT` / `TEXT_MESSAGE_CHUNK` | assistant text scanned for secret patterns and SSRF URLs (both the enveloped and self-contained wire forms) | **Yes** — redact (literal or regex); an SSRF URL drops the chunk |
| `TOOL_CALL_RESULT` | tool-result content scanned for secret patterns and SSRF URLs | **Yes** — redact (literal or regex; the indirect-injection / exfiltration surface); an SSRF URL drops the result |
| `STATE_SNAPSHOT` / `STATE_DELTA` | size budget; a `STATE_DELTA` is validated as a well-formed RFC 6902 patch and bounded (op count, pointer depth, per-op value size); payload scanned for secret markers | **Yes** — a malformed patch fails closed, an over-budget patch is rejected, and a secret marker is denied (a structured payload can't be masked, so it's blocked, not leaked) |
| Lifecycle (`RUN_*`, `STEP_*`) | ordering enforced by the `Run` state machine | structural only (no policy verdict) |
| Request leg (`RunAgentInput`) | `tools[*].name` authorized; `user` **and `system`** message text, plus the `context` / `forwardedProps` / inbound `state` JSON, screened for secret markers and SSRF URLs (domain hosts **resolved** and re-checked, closing DNS-rebinding) | **Yes** — reject before forwarding |
| Malformed **known** events | a recognized type with a missing/blank required field cannot be inspected → handled per `[policy].on_malformed_event` (default `terminate`) | **Yes** — fails closed by default |

## What is forwarded uninspected

| Event / input | Why it matters |
|---|---|
| **State** patch *semantics* | a `STATE_DELTA` is validated (well-formed RFC 6902) and bounded, but the proxy does not model the resulting document — it cannot reason about which paths an op *should* be allowed to touch |
| `RAW`, `CUSTOM`, `REASONING_ENCRYPTED_VALUE` | opaque — forwarded as-is, never inspected |
| Unknown / future AG-UI event types | forwarded raw |

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
- **Audit completeness:** records are queued to a bounded outbox whose fill is
  exported (`agate_audit_outbox_depth` / `_capacity`). On a full outbox the
  `[audit].outbox_on_full` policy decides: `block` (default — backpressure the
  proxy, never lose a record) or `shed` (drop, loudly logged and counted, never
  silent). Monitor the depth gauge and the drop counter.
- **Cross-run replay memory:** off by default; enable `[policy.session_memory]`
  to quarantine a tool (by name) for the rest of a session once it is denied,
  so the agent cannot retry it with varied arguments in a later run. It only
  ever *adds* a denial over the stateless policy. The ledger is process-local
  with a sliding TTL by default, or **Redis** (`backend = "redis"`) shared
  across replicas and restarts; the Redis backend fails open — an unreachable
  Redis degrades to no memory, never a wrong allow.
- **TLS** is terminated at the proxy (required to inspect plaintext); it is off
  by default and configured under `[tls]`.

## Roadmap

The remaining work is forward-looking — semantic state-path allowlisting, a
plugin policy engine, and sub-IP / per-API-key rate limits — and is sequenced in
[`security-coverage-roadmap.md`](https://github.com/C3EQUALZz/agate/blob/main/docs/design/security-coverage-roadmap.md).
(Malformed-event fail-closed, tool-argument deny rules, secret redaction of tool
results and state, per-run response budgets, SSRF screening on both legs with
DNS-rebinding resolution, RFC 6902 patch validation/bounding, audit-outbox
backpressure signalling with a block/shed policy, and cross-run per-session
replay memory — process-local or Redis-backed — are now implemented.)
