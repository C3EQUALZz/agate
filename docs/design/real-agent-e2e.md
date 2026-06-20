# Design: real-agent end-to-end validation

> Status: **accepted (record of a validation pass)**. Once the security-coverage
> roadmap, the high-load audit, and the refactoring revision were done, the
> proxy was driven end-to-end in front of a **real** AG-UI agent (the bundled
> `examples/ag-ui-agent`, an AG2/`autogen.beta` agent backed by a live LLM)
> rather than a stub. Exercising real traffic surfaced three behavioural gaps
> that the stub-based tests could not — all now fixed. This document records the
> setup, what was validated, the findings, and how to reproduce the run.

## Why a real agent

The test suite drives a *stub* SSE agent emitting a fixed, well-formed event
sequence. A real agent differs in ways that matter to an inspecting proxy:

- it identifies the run by an AG-UI `threadId` the client supplies;
- it streams assistant text as `TEXT_MESSAGE_CHUNK` (the self-contained form),
  not the enveloped `TEXT_MESSAGE_CONTENT` the tests used;
- it makes **concurrent** tool calls and terminates them with
  `TOOL_CALL_RESULT` rather than `TOOL_CALL_END`.

Each of those is a real input shape the proxy must handle correctly. None was
covered by the stub, so each hid a bug.

## Findings (all fixed)

| # | Gap | Fix |
|---|-----|-----|
| 1 | `SessionId`/`RunId` were minted as random `Uuid::new_v4()` per request, so per-session replay memory (G2) could never fire across runs and audit ids did not correlate to the conversation. | PR #93 — derive them deterministically (UUIDv5) from the AG-UI `threadId`/`runId`. |
| 2 | Only `TEXT_MESSAGE_CONTENT` was inspected; the agent's `TEXT_MESSAGE_CHUNK` fell through to opaque pass-through, so secret redaction and response-leg SSRF screening of assistant text were silently bypassed. | PR #94 — the AG-UI adapter normalizes both wire forms to one domain `MessageChunk`. |
| 3 | Tool-call `START`/`ARGS` frames were held in one flat buffer flushed by any `Forward`; with concurrent, `END`-less calls an allowed call's result leaked a *denied* call's frames to the client, and an `END`-less call was never judged. | PR #95 — buffer per call id, gate each call on its own verdict, and sweep calls left open at run end. |

The common thread: the stub never reproduced these shapes, so unit tests passed
while the deployed proxy mishandled real traffic. Each fix shipped with
regression tests in the proxy's own suite so the shape is now covered without a
live agent.

## Controls validated against the live agent

Every shipped control was exercised end-to-end (client → agate → agent → LLM):

- **Tool authorization (request leg)** — offering a non-allowlisted tool is
  rejected `403` before the agent runs.
- **SSRF screen (request leg)** — a metadata-IP URL in a user message is
  rejected `403`.
- **Secret screen (request leg)** — a configured marker in a user message is
  rejected `403`.
- **Redaction (response leg)** — a literal marker and a regex pattern
  (`sk-…`) the model emits are masked to `[REDACTED]` before the client.
- **Result deny rules** — a `TOOL_CALL_RESULT` whose content matches a
  `[[policy.tools.deny_results]]` rule is dropped before the client.
- **Per-call tool-frame gating** — a denied tool call's frames (name +
  arguments) never reach the client even when concurrent and `END`-less.
- **Per-session identity** — the same `threadId` maps to the same `SessionId`
  across runs (the prerequisite for replay memory).
- **Audit** — every inspected event and verdict is appended to the
  transparency log (verified in Postgres).

## Reproducing the run

The agent and the proxy are configured independently.

1. **Point the example agent at a provider.** Copy `examples/ag-ui-agent/.env`
   from `.env.example`; for an OpenAI-compatible provider (e.g. Mistral) set
   `AGENT__OPENAI_BASE_URL`, `AGENT__OPENAI_API_KEY`, `AGENT__OPENAI_MODEL`.
   Run it: `uv run uvicorn ag_ui_agent.main.entrypoint:create_app --factory
   --host 127.0.0.1 --port 8000`.
2. **Start an audit store:** any PostgreSQL (the server applies its migrations
   on boot).
3. **Configure the proxy** (`agate.toml`): `[proxy].agent_endpoint =
   "http://127.0.0.1:8000/api/run"`, `[audit].database_url = …`, and the
   `[policy]` rules to exercise. Run `agate-server` (`AGATE_CONFIG=…`).
4. **Drive it:** `POST /` on the proxy with a `RunAgentInput`
   (`{"threadId":"…","runId":"…","messages":[…],"tools":[…]}`) and read the
   streamed SSE response.

This is a manual validation, not a committed test — it needs a live LLM key. The
behaviours it found are each covered by an in-repo regression test (see the
proxy `stream`/`run_inspection`/`ag_ui` suites and the server e2e), so CI guards
them without a provider.
