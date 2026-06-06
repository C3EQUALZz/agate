# Agate examples — an AG-UI agent behind Agate

Runnable **Python** examples that put an [AG-UI](https://docs.ag-ui.com/) agent
**behind Agate** and show Agate's protections — tool-call denial, secret
redaction, and the tamper-evident audit trail — in action.

Agate is an **inline reverse proxy** for LLM agents. The client talks to Agate
(`POST http://localhost:8080/`), Agate forwards the run to the real agent's
AG-UI endpoint, and streams the agent's SSE response back **after inspecting
it**: it can deny tool calls not on an allowlist, redact secret markers from
emitted text, and record every decision to an append-only transparency log.

```
client ──POST /──▶  Agate (:8080)  ──forward──▶  AG-UI agent (:8000/run)
                       │  inspect each SSE event (allow / deny / redact)
                       └──────────▶  Postgres transparency log
client ◀──inspected SSE stream──  Agate
```

## The examples

| Example | What it shows | Needs |
| --- | --- | --- |
| [`agent-basic/`](agent-basic/) | A minimal AG-UI agent (FastAPI + SSE) built with **ag2** (AutoGen 2) and **dishka** dependency injection. Runs in a **stub mode** with no API key, or a **real ag2 mode** with `OPENAI_API_KEY`. This is the upstream Agate sits in front of. | uv |
| [`protected-demo/`](protected-demo/) | A `docker-compose` running the agent + **Agate** + **Postgres**, wired so Agate **denies** a dangerous `delete_file` tool (allowlist = `["search"]`) and **redacts** a secret marker. A client script sends a run through Agate and prints the streamed events so you *see* the tool call dropped and the secret masked. | uv + Docker |
| [`audit-verify/`](audit-verify/) | Walks through the transparency log: queries Postgres to show that every `(event, verdict)` decision was recorded. | Docker (uses the demo's Postgres) |

Start at **`agent-basic/`** to understand the agent, then run
**`protected-demo/`** to see Agate protect it.

## Prerequisites

- [**uv**](https://docs.astral.sh/uv/) — Python packaging/runner
  (`curl -LsSf https://astral.sh/uv/install.sh | sh`).
- **Docker** + the Compose plugin (for `protected-demo` and `audit-verify`).

Each example is a self-contained [uv](https://docs.astral.sh/uv/) project with a
**src-layout** (`pyproject.toml`, `src/<pkg>/`) and its own `README.md` with
exact run commands.

## A note on package versions

These examples target a **fast-moving, partly-beta** ecosystem (AG2's `autogen`
beta line, the AG-UI Python SDK, `dishka-ag2`). Versions are constrained but not
hard-pinned, and a few API signatures are flagged inline with
`# VERIFY:` comments. After `uv sync`, run the example and adjust those spots if
an API has shifted. See each project's README for the specifics.
