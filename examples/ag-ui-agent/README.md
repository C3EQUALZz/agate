# ag-ui-agent

A clean-layered **AG2.beta + AG-UI + Dishka** FastAPI agent — the upstream that
**[Agate](../../README.md)** sits in front of and protects. It is modeled
precisely on the public reference
[`vvlrff/ag2_ag-ui_example`](https://github.com/vvlrff/ag2_ag-ui_example):
same Clean-Architecture layering, the same `import-linter` contracts, the same
single `dishka-ag2` + `AGUIStream` wiring.

The agent manages a small **document workspace** and exposes these tools:

| Tool | Kind | Why it's here |
| --- | --- | --- |
| `search_documents` | safe, read-only | the tool Agate's allowlist **permits** |
| `list_documents` | safe, read-only | another read-only capability |
| `delete_file` | **dangerous**, destructive | the tool Agate **denies** in the demo |
| `echo_status` | trivial, no deps | a no-dependency demo tool |

It also emits assistant text containing a fake `sk-...` credential, so Agate's
**secret redaction** is demonstrable. See [`../protected-demo`](../protected-demo)
to watch Agate block the dangerous call and mask the secret.

## One backend: real AG2 over OpenAI

There is a single backend — a real `autogen.beta.Agent` over OpenAI, streamed via
`autogen.beta.ag_ui.AGUIStream`, with its tools injected through Dishka by
`dishka-ag2`. **An OpenAI API key is required; there is no offline mode.**

```bash
uv sync
export AGENT__OPENAI_API_KEY=sk-...        # your real key
uv run ag-ui-agent                          # serves on http://0.0.0.0:8000
# or: uv run uvicorn --factory ag_ui_agent.main.entrypoint:create_app --reload
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

The system prompt (`api/agent/prompts.py`) is deliberately firm: it instructs the
model to call `search_documents`, then attempt the dangerous `delete_file`, then
emit a fake `sk-...` token — so a single run exercises every Agate protection.
**Whether a given model follows every step depends on the model**; `gpt-4o-mini`
follows it reliably. The protected-demo's expectations assume this behaviour.

## How the dishka-ag2 + AGUIStream wiring works

This is the exact pattern from the reference:

1. **One `AG2Scope` container** (`main/di.py`). `dishka-ag2`'s `AG2Scope.APP`
   holds singletons; `AG2Scope.REQUEST` is opened on every HTTP request *and*
   every agent tool call. Providers are registered per concern (settings,
   repositories, use cases, toolkit, agent) plus `AG2Provider()`.
2. **The toolkit is assembled by Dishka** (`main/providers/toolkit.py`): a
   `ToolkitProvider` builds the `Toolkit` from the `@tool @inject` functions and
   provides it at `AG2Scope.APP`. The agent factory resolves a ready `Toolkit`
   from the container instead of hand-constructing it.
3. **The agent is an `AG2Scope.APP` singleton** built by a factory that receives
   the live container by injection (`main/providers/agent.py`):
   ```python
   @provide(scope=AG2Scope.APP)
   def provide_agent(self, config, toolkit, container) -> Agent:
       return build_agent(config, toolkit, container)   # attaches DishkaAsyncMiddleware
   ```
   Letting dishka inject the container breaks the agent⇄container cycle without a
   second container.
4. **Tools are plain `@tool @inject` functions** (`api/agent/tools/documents.py`):
   ```python
   @tool
   @inject
   async def search_documents(uc: FromDishka[SearchDocumentsUseCase], query: str, ...): ...
   ```
   `DishkaAsyncMiddleware` opens an `AG2Scope.REQUEST` child container before the
   tool runs and resolves the `FromDishka[...]` parameters from it.
5. **A tiny ASGI middleware** (`main/middleware.py`) opens `AG2Scope.REQUEST` per
   HTTP request so `dishka.integrations.fastapi`'s `@inject` keeps working for
   the REST routes.
6. **The run endpoint** (`api/routes/chat.py`) resolves the `AgUiStreamer` port
   and streams it — concretely `AGUIStream(agent).dispatch(run_input, accept=accept)`.

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
│       ├── streamer.py          # AgUiStreamer port (the backend implements it)
│       ├── run_input.py         # framework-neutral RunAgentInput model
│       ├── prompts.py
│       ├── tools/               # AG2 tools via FromDishka[UseCase]
│       └── backends/ag2.py      # build_agent + AGUIStream(agent).dispatch
└── main/
    ├── entrypoint.py            # create_app() / build_app(); uvicorn factory mode
    ├── di.py                    # the single AG2Scope container
    ├── middleware.py            # AG2ContainerMiddleware
    └── providers/{settings,repositories,usecases,toolkit,agent}.py
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
`AG2Scope.APP`-scoped singleton so writes persist across turns (a real DB adapter
would be request-scoped, as in the reference).

## Architecture invariants

The same three `import-linter` contracts as the reference (`.importlinter`):

1. **Layer direction** — `main → api → usecases → gateways → models → domain`.
2. **Agent framework isolation** — `autogen` / `dishka_ag2` imports never leak
   below `api.agent` / `main.providers`.
3. **FastAPI isolation** — `fastapi` / `starlette` stay out of `domain` /
   `gateways` / `usecases`.

```bash
uv run lint-imports
```

## Quality gate

A strict toolchain — run all of it (or `just gate`):

```bash
uv run ruff check src tests          # ruff at select = ALL (curated ignores)
uv run ruff format --check src tests
uv run flake8 src tests              # wemake-python-styleguide (WPS); config in setup.cfg
uv run lint-imports                  # the .importlinter contracts
uv run mypy src tests                # maximum strictness
uv run pytest                        # unit + integration (a fake streamer; no OpenAI call)
```

The tests never call OpenAI: the chat route is driven in tests by a fake
`AgUiStreamer` (`tests/fakes/streamer.py`) that reproduces the demo's AG-UI event
sequence while running the real `search_documents` use case.

> Note on versions: the AG2 path targets a fast-moving, partly-beta ecosystem
> (`autogen.beta`, the AG-UI SDK, `dishka-ag2`). A few signatures on that path
> are flagged inline with `# VERIFY:` — run the agent once and adjust if an API
> has shifted.
