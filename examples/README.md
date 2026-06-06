# Agate examples — a clean-architecture AG-UI agent behind Agate

Runnable **Python** examples that put an [AG-UI](https://docs.ag-ui.com/) agent
**behind Agate** and show Agate's three protections — tool-call **denial**,
secret **redaction**, and the tamper-evident **audit trail** — in action.

Agate is an **inline reverse proxy** for LLM agents. The client talks to Agate
(`POST http://localhost:8080/`), Agate forwards the run to the real agent's
AG-UI endpoint, and streams the agent's SSE response back **after inspecting
it**: it can deny tool calls not on an allowlist, redact secret markers from
emitted text, and record every decision to an append-only transparency log.

```
client ──POST /──▶  Agate (:8080)  ──forward──▶  AG-UI agent (:8000/api/run)
                       │  inspect each SSE event (allow / deny / redact)
                       └──────────▶  Postgres transparency log
client ◀──inspected SSE stream──  Agate
```

## The examples

| Example | What it shows | Needs | Maps to Agate's… |
| --- | --- | --- | --- |
| [`ag-ui-agent/`](ag-ui-agent/) | The flagship. A **clean-layered** AG2.beta + AG-UI + **Dishka** FastAPI agent, modeled precisely on [`vvlrff/ag2_ag-ui_example`](https://github.com/vvlrff/ag2_ag-ui_example) (domain → models → gateways → usecases → api → main, `import-linter` contracts, `dishka-ag2` + `AGUIStream`). Runs in an offline **stub** backend (no key) or a real **ag2** backend. Exposes a safe `search_documents`, a dangerous `delete_file`, and a secret-leaking text path. | uv | the agent Agate fronts |
| [`protected-demo/`](protected-demo/) | A `docker-compose` running the agent + **Agate** + **Postgres**, wired so Agate **denies** `delete_file` (allowlist = `["search_documents"]`) and **redacts** an `sk-...` marker. A layered client posts a run through Agate and prints the stream so you *see* the dangerous call dropped, the secret masked, the safe call pass. | uv + Docker | **tool denial** + **redaction** |
| [`audit-verify/`](audit-verify/) | Reads Agate's transparency log from Postgres to show every `(event, verdict)` decision was recorded in a gapless Merkle leaf sequence. | uv + the demo's Postgres | the **audit trail** |

Start at **`ag-ui-agent/`** to understand the agent, then run
**`protected-demo/`** to see Agate protect it, then **`audit-verify/`** to prove
it was all recorded.

## How each example maps to an Agate protection

- **Tool-call denial.** The agent offers `delete_file`; the demo's
  `[policy.tools] mode="allowlist" names=["search_documents"]` means Agate denies
  it (surfaced as `RUN_ERROR`) while letting `search_documents` through.
- **Secret redaction.** The agent emits an `sk-...` token in assistant text; the
  demo's `[policy] redact=["sk-","AKIA"]` masks it to `[REDACTED]`.
- **Transparency log.** Every inspected `(event, verdict)` is appended to the
  Postgres-backed Merkle log; `audit-verify` shows the gapless leaf sequence.

## Prerequisites

- [**uv**](https://docs.astral.sh/uv/) — Python packaging/runner
  (`curl -LsSf https://astral.sh/uv/install.sh | sh`).
- **Docker** + the Compose plugin (for `protected-demo` and `audit-verify`).

Each example is a self-contained [uv](https://docs.astral.sh/uv/) project with a
**src-layout** (`uv_build` backend, `src/<pkg>/`) and its own `README.md` with
exact `uv run` commands.

## A note on package versions

The `ag-ui-agent`'s real-**ag2** backend targets a fast-moving, partly-beta
ecosystem (AG2's `autogen.beta`, the AG-UI SDK, `dishka-ag2`). Those signatures
are constrained but not hard-pinned, and a few are flagged inline with
`# VERIFY:`. The default **stub** backend has none of those dependencies, needs
no key, and is the path the tests and the protected demo exercise — so the demo
is reproducible fully offline. After `uv sync --extra ag2`, run the ag2 backend
once and adjust any `# VERIFY:` spot if an API has shifted.
