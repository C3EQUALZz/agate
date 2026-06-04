# agate-proxy — Threat Model & Deployment Topology

Status: **accepted** (initial design)
Scope: the `agate-proxy` bounded context — the data plane that inspects LLM-agent
traffic. This document defines what `agate-proxy` defends, against whom, where it
sits, and the single decision seam (event → verdict) that the audit and policy
contexts plug into.

---

## 1. Context

Agate is a security gateway for LLM agents speaking the **AG-UI** protocol (with
**AG2** as the reference agent framework). `agate-proxy` is the reverse proxy in
the request path. It adds authentication, input validation, output inspection,
and a decision point — **without changing agent code**.

The core is **protocol-agnostic**; AG-UI is the first transport adapter. A second
adapter (agent ↔ LLM provider API) can be added later without touching the core
inspection domain.

---

## 2. Protocol-derived attack surface

These facts about AG-UI drive the threat model. AG-UI is HTTP `POST` of a
`RunAgentInput` JSON body (client → agent) plus a stream of events
(`text/event-stream`, SSE) in the response (agent → client). 34 event types
across lifecycle, text streaming, tool calls, and state management.

What the protocol does **not** provide — and therefore what the proxy must add:

| Gap in AG-UI | Consequence | Proxy must |
| --- | --- | --- |
| No authentication / authorization | Anyone who reaches the endpoint can drive an agent | Enforce authn/authz on the POST before streaming |
| Untyped `any` everywhere (`state`, `forwardedProps`, `context.value`, tool `parameters`, `RAW`, `CUSTOM`) | Schema confusion, oversized payloads, injection | Cap sizes; schema-check where a schema exists; treat opaque fields as untrusted |
| `STATE_DELTA` = unbounded JSON Patch (RFC 6902) | State poisoning; DoS via op count/depth/value size | Validate and bound patch operations |
| No sequence numbers, no per-event signature, optional `timestamp` | Replay; forged-event injection if a leg is compromised | Rely on ordered transport (single-connection SSE); add own ordering/idempotency where needed |
| Tool-call args streamed as concatenated JSON-string fragments (`TOOL_CALL_ARGS` between `START`/`END`) | A decision cannot be made on a single frame | Buffer the full tool-call before issuing a verdict |
| `user` messages may embed remote URLs (image/document sources) | SSRF / content-fetch surface | Screen URL-typed input content |
| `encryptedValue`, `REASONING_ENCRYPTED_VALUE`, `RAW`, `CUSTOM` are opaque | Cannot be inspected | Policy of pass-through-or-drop; never trust |
| Long-lived streams, no size limits | Resource exhaustion, slowloris | Time/size/rate budgets per run and per connection |
| Optional binary protobuf transport variant | Parser confusion | Negotiate/restrict accepted encodings |

The AG-UI architecture doc itself anticipates an optional "Secure Proxy"
middlebox but specifies nothing about what it must enforce — that gap is the
contribution of this work.

---

## 3. Assets (what we protect)

1. **Tool-call authorization** — which tools, with which arguments, an agent may invoke.
2. **Sensitive data** in `messages`/`state` (PII, secrets) — against exfiltration.
3. **Agent instruction integrity** — resistance to prompt injection, including
   indirect injection via fetched URL content and tool results.
4. **Shared state integrity** — against poisoning through `STATE_DELTA`.
5. **Availability** of the agent service — against DoS (oversized state/patches, slow streams).
6. **Audit-trail tamper-evidence** — already owned by `agate-audit`; the proxy
   feeds it but does not re-implement it.

---

## 4. Trust boundaries & actors

Path: `frontend ↔ agate-proxy ↔ agent app (AG2) ↔ LLM provider ↔ tools / MCP servers`.

The proxy sits on the **frontend ↔ agent** boundary first (AG-UI). Everything on
the client side of the proxy, and everything emitted by the agent/LLM, is
**untrusted input** to be inspected.

Threat actors:

- **Malicious / compromised client (frontend)** — crafts `RunAgentInput`:
  oversized `state`, malicious tool parameters, SSRF URLs, prompt injection in
  `user` messages.
- **Malicious / compromised agent or LLM backend** — emits hostile events:
  exfiltration via tool calls, state poisoning via `STATE_DELTA`, harmful
  content, unauthorized tool invocations.
- **Compromised tool / MCP server** — when tool results are proxied back.
- **Privileged operator / insider** — log tampering (mitigated by `agate-audit`,
  out of scope here).
- **Indirect prompt injection** — data pulled via URL sources or tool results
  that manipulates the agent.

Out of scope (handled elsewhere or by infrastructure): transport-level MitM
(TLS), the agent's own internal logic, the LLM provider's safety.

---

## 5. Threat enumeration

Tailored STRIDE, mapped to AG-UI specifics.

- **Spoofing** — no protocol auth → impersonating a user/agent. *Mitigation:*
  authn on the POST; the proxy is the TLS endpoint (§6).
- **Tampering** — forged/modified events on a compromised leg; `STATE_DELTA`
  poisoning. *Mitigation:* ordered single-connection transport; bounded,
  validated patches; verdict on state-mutating events.
- **Repudiation** — denying an action. *Mitigation:* every inspected event +
  verdict recorded to the audit transparency log.
- **Information disclosure** — PII/secret exfiltration via tool-call args or
  text content; SSRF via URL sources. *Mitigation:* buffer-and-inspect tool
  calls; redact text content; screen URLs.
- **Denial of service** — oversized `state`, unbounded JSON Patch, slowloris on
  the SSE stream. *Mitigation:* size/time/rate budgets; reject early on the
  request leg.
- **Elevation of privilege** — invoking tools beyond the client's grant.
  *Mitigation:* tool-call allow/deny verdict at the seam (policy fills this in).

---

## 6. Deployment topology (decisions)

### 6.1 Placement — protocol-agnostic core, AG-UI adapter first

The inspection **core is protocol-agnostic**; the wire protocol enters through an
adapter that translates wire events into domain events. The **AG-UI adapter is
built first** (primary position: `frontend ↔ agent`). A second adapter for
`agent ↔ LLM` traffic can be added later with no change to the core — this is the
concrete proof of "AG-UI is one adapter."

### 6.2 Mode — hybrid inline

The proxy is **inline** (in the request path, able to block/transform), operating
in two phases:

- **Request leg (preventive):** the full `RunAgentInput` is available before
  forwarding, so validation/authz/size-limits are cheap and decisive — reject
  before the agent ever runs.
- **Response leg (streaming inspection):** the SSE event stream is parsed
  incrementally; the proxy can **terminate** the stream, **replace** it with a
  `RUN_ERROR`, or **redact**/transform content (e.g. `TEXT_MESSAGE_CONTENT`).
  Tool calls are buffered between `TOOL_CALL_START` and `TOOL_CALL_END` so a
  verdict sees complete arguments.

Fail-open vs fail-closed on policy is **configurable per deployment**. This is
also where the evaluation chapter comes from: added latency and throughput cost
of inline streaming inspection.

(Rejected: *inline-only preventive* — cannot inspect streamed output well;
*detective tap* — zero latency but cannot prevent anything, so it can't satisfy
assets 1–4. Hybrid keeps prevention while bounding the cost.)

### 6.3 TLS — terminated at the proxy

`agate-proxy` is the **TLS endpoint** the frontend connects to. Terminating TLS
is required for inline content inspection. (An external LB may still front it,
but the proxy must see plaintext to inspect.)

---

## 7. The event → verdict seam

Inspection produces, per event (or per buffered logical unit), a **verdict**:

```
Allow                  // forward unchanged
Deny(reason)           // block; on the response leg, surface as RUN_ERROR
Transform(replacement) // forward a modified event (e.g. redacted content)
Buffer                 // need more frames before deciding (e.g. mid tool-call)
Terminate(reason)      // end the run/stream
```

This single seam is where **two contexts plug in**:

- **`agate-audit`** — records `(event, verdict)` to the transparency log.
- **`agate-policy`** — *computes* the verdict (a `PolicyPort`).

For the first milestone, `agate-proxy` ships a trivial **allow-all** policy
adapter behind `PolicyPort`; `agate-policy` replaces it later without changing
the data plane.

---

## 8. Out of scope (for now)

- The `agent ↔ LLM` adapter (designed-for, not yet implemented).
- Policy content (allowlists, PII redaction rules, anti-injection heuristics) —
  owned by `agate-policy`.
- External anchoring of audit checkpoints — owned by `agate-audit`.
- Operator-side log tampering — mitigated by the audit context's design.

---

## 9. Next steps

1. Define the `agate-proxy` domain: a `Session`/`Run` aggregate, protocol-agnostic
   `InspectedEvent` value objects, and the `Verdict` value object.
2. Define application ports: `PolicyPort` (verdict source), an audit sink, and the
   upstream agent client.
3. Build the AG-UI adapter: SSE codec (incremental, order-preserving),
   `RunAgentInput` validation, event translation.
4. HTTP/SSE presentation (axum/hyper), TLS termination, request/response wiring.
5. Evaluation harness: latency/throughput overhead of inline inspection.
