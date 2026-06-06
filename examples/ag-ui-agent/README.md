# ag-ui-agent

A clean-layered **AG2.beta + AG-UI + Dishka** FastAPI agent — the upstream that
**[Agate](../../README.md)** sits in front of and protects. It is modeled
precisely on the public reference
[`vvlrff/ag2_ag-ui_example`](https://github.com/vvlrff/ag2_ag-ui_example):
same Clean-Architecture layering, the same `import-linter` contracts, the same
`dishka-ag2` + `AGUIStream` wiring.

The agent manages a small **document workspace** and exposes three tools:

| Tool | Kind | Why it's here |
| --- | --- | --- |
| `search_documents` | safe, read-only | the tool Agate's allowlist **permits** |
| `list_documents` | safe, read-only | another read-only capability |
| `delete_file` | **dangerous**, destructive | the tool Agate **denies** in the demo |

It also emits assistant text containing a fake `sk-...` credential, so Agate's
**secret redaction** is demonstrable. See [`../protected-demo`](../protected-demo)
to watch Agate block the dangerous call and mask the secret.

## Two backends

The agent runs in either of two modes, selected by `AGENT__BACKEND`:

- **`stub`** (default) — a scripted, deterministic AG-UI stream. **No API key,
  no autogen/OpenAI dependencies.** It drives the *real* use cases (so the
  workspace genuinely mutates) and emits a fixed sequence: a safe `search`
  call, a dangerous `delete_file` call, and a secret-leaking text message. This
  makes the whole demo reproducible fully offline.
- **`ag2`** — a real `autogen.beta.Agent` over OpenAI, streamed via
  `autogen.beta.ag_ui.AGUIStream`, with tools injected through Dishka by
  `dishka-ag2`. Requires the `ag2` extra and an OpenAI key.

Both implement one `AgUiStreamer` port, so the HTTP route — and Agate in front
of it — sees an identical AG-UI SSE contract. Switching backend is a one-line DI
change; no route changes.

## Quickstart (offline stub — no key)

```bash
uv sync
uv run ag-ui-agent          # serves on http://127.0.0.1:8000
# or: uv run uvicorn ag_ui_agent.main.entrypoint:app --reload
```

Smoke-test:

```bash
curl http://127.0.0.1:8000/api/health                 # {"status":"healthy"}
curl http://127.0.0.1:8000/api/documents              # the seeded workspace

# The AG-UI run endpoint (this is what Agate forwards to):
curl -N -X POST http://127.0.0.1:8000/api/run \
  -H 'Content-Type: application/json' \
  -H 'Accept: text/event-stream' \
  -d '{"threadId":"t1","runId":"r1",
       "messages":[{"id":"m1","role":"user","content":"find the api key"}],
       "state":{},"context":[],"tools":[],"forwardedProps":{}}'
```

Expect an SSE stream:
`RUN_STARTED → TOOL_CALL_START(search_documents) → … → TOOL_CALL_START(delete_file) → … → TEXT_MESSAGE_CONTENT(… sk-… …) → RUN_FINISHED`.

## Running the real AG2 backend

```bash
uv sync --extra ag2
export AGENT__BACKEND=ag2
export AGENT__OPENAI_API_KEY=sk-...        # your key
uv run uvicorn ag_ui_agent.main.entrypoint:app --reload
```

## How the dishka-ag2 + AGUIStream bridge works (ag2 backend)

This is the exact pattern from the reference:

1. **One `AG2Scope` container.** `dishka-ag2`'s `AG2Scope.APP` holds singletons;
   `AG2Scope.REQUEST` is opened on every HTTP request *and* every agent tool
   call.
2. **The agent is an APP-scoped singleton** built by a factory that receives the
   live container by injection (`main/providers/agent_ag2.py`):
   ```python
   @provide(scope=AG2Scope.APP)
   def provide_agent(self, config: OpenAIConfig, container: AsyncContainer) -> Agent:
       return build_agent(config, container)   # attaches DishkaAsyncMiddleware
   ```
   Letting dishka inject the container breaks the agent⇄container cycle without
   a second container.
3. **Tools are plain `@tool @inject` functions** (`api/agent/tools/documents.py`):
   ```python
   @tool
   @inject
   async def search_documents(uc: FromDishka[SearchDocumentsUseCase], query: str, ...): ...
   ```
   `DishkaAsyncMiddleware` opens an `AG2Scope.REQUEST` child container before the
   tool runs and resolves the `FromDishka[...]` parameters from it.
4. **A tiny ASGI middleware** (`main/middleware.py`) opens `AG2Scope.REQUEST` per
   HTTP request so `dishka.integrations.fastapi`'s `@inject` keeps working for
   the REST routes.
5. **The run endpoint** (`api/routes/chat.py`) resolves the `AgUiStreamer` port
   and streams it — for the ag2 backend that is
   `AGUIStream(agent).dispatch(run_input, accept=accept)`.

## Architecture

```
src/ag_ui_agent/
├── config.py / logging_config.py
├── domain/entities/             # Document — a pure frozen-ish dataclass
├── models/                      # storage-shaped seed data (see "Why in-memory")
├── gateways/db/document/        # DocumentRepository (Protocol) + InMemory adapter
├── usecases/document/           # Request/Response use cases (search/list/get/delete)
├── api/
│   ├── middlewares/request_id.py
│   ├── schemas/document.py
│   ├── routes/{health,documents,chat}.py   # chat = POST /api/run (AG-UI SSE)
│   └── agent/
│       ├── streamer.py          # AgUiStreamer port (both backends implement it)
│       ├── run_input.py         # framework-neutral RunAgentInput model
│       ├── sse.py / prompts.py
│       ├── tools/               # AG2 tools via FromDishka[UseCase]  (ag2 only)
│       └── backends/
│           ├── stub.py          # scripted offline streamer
│           └── ag2.py           # AGUIStream(agent).dispatch  (ag2 only)
└── main/
    ├── entrypoint.py            # create_app(): branches stub vs ag2
    ├── di.py                    # stub (Scope) vs ag2 (AG2Scope) containers
    ├── middleware.py            # AG2ContainerMiddleware (ag2 only)
    └── providers/{settings,repositories,usecases,agent,agent_ag2}.py
```

### Why in-memory instead of Postgres

The reference persists notes in Postgres via SQLAlchemy + Alembic. This example
**deliberately uses an in-memory document store** so it runs with zero external
infrastructure — important because its whole job is to be the easy-to-launch
upstream for the Agate demo. The layering is preserved exactly: the `gateways`
layer still defines a `DocumentRepository` **port** (Protocol) and an adapter
(`InMemoryDocumentRepository`); use cases depend only on the port. Swapping in a
real `AlchemyDocumentRepository` would be a one-file adapter change plus a
request-scoped session provider — no use-case or route changes. The store is an
`APP`-scoped singleton so writes persist across turns (a real DB adapter would be
request-scoped, as in the reference).

## Architecture invariants

The same three `import-linter` contracts as the reference (`.importlinter`):

1. **Layer direction** — `main → api → usecases → gateways → models → domain`.
2. **Agent framework isolation** — `autogen` / `dishka_ag2` imports never leak
   below `api.agent` / `main.providers.agent`.
3. **FastAPI isolation** — `fastapi` / `starlette` stay out of `domain` /
   `gateways` / `usecases`.

```bash
uv run lint-imports
```

## Quality gate

```bash
uv run ruff check src tests && uv run ruff format --check src tests
uv run lint-imports
uv run mypy src tests
uv run pytest                 # unit + integration (stub backend; no infra)
```

> Note on versions: the `ag2` extra targets a fast-moving, partly-beta ecosystem
> (`autogen.beta`, the AG-UI SDK, `dishka-ag2`). A few signatures on that path
> are flagged inline with `# VERIFY:` — run the ag2 backend once and adjust if an
> API has shifted. The **stub** backend has no such dependencies and is the path
> the tests and the protected demo exercise.
