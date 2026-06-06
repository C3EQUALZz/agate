# agent-basic — a minimal AG-UI agent (ag2 + dishka)

The upstream agent that **Agate** sits in front of. It exposes an
**AG-UI-compatible SSE endpoint** at `POST /run`: send a `RunAgentInput` JSON
body, receive a `text/event-stream` of AG-UI events (`RUN_STARTED`,
`TEXT_MESSAGE_CONTENT`, `TOOL_CALL_START/ARGS/END`, `RUN_FINISHED`, ...).

Two backends:

| `AGENT_BACKEND` | Needs | What it does |
| --- | --- | --- |
| `stub` (default) | nothing | Emits a fixed, illustrative event script — assistant text containing a fake secret, an allowed `search` tool call, and a dangerous `delete_file` tool call. Deterministic and offline. |
| `ag2` | `OPENAI_API_KEY` | A real [AutoGen 2](https://docs.ag2.ai/) `ConversableAgent` bridged to AG-UI via `autogen.ag_ui.AGUIStream`, with a `get_weather` tool. |

The **stub** is what the [`protected-demo`](../protected-demo/) uses, so the
demo runs without any API key and shows Agate's protections reproducibly.

## Layout (uv, src-layout)

```
agent-basic/
  pyproject.toml          # uv project; deps + optional [ag2] extra
  src/agent_basic/
    ag_ui.py              # AG-UI event constructors + SSE framing (matches Agate)
    run_input.py          # RunAgentInput parsing
    config.py             # typed env config
    providers.py          # dishka DI providers
    backends/
      base.py             # AgentBackend port (Protocol)
      stub.py             # scripted, key-free backend
      ag2_backend.py      # real ag2 agent via AGUIStream
    app.py                # FastAPI app: POST /run, dishka-wired
    __main__.py           # uvicorn entrypoint
```

## Run it (stub mode — no API key)

```bash
cd examples/agent-basic
uv sync                       # create venv, install fastapi/uvicorn/dishka
uv run agent-basic            # serves on http://0.0.0.0:8000
```

In another terminal, hit the AG-UI endpoint directly (this is what Agate does):

```bash
curl -N -X POST http://localhost:8000/run \
  -H 'content-type: application/json' \
  -d '{"threadId":"t1","runId":"r1","messages":[{"role":"user","content":"hi"}]}'
```

You will see SSE frames stream back, including the secret `sk-…` in the text and
both the `search` and `delete_file` tool calls — **unprotected**. Putting Agate
in front (see [`protected-demo`](../protected-demo/)) is what redacts the secret
and drops `delete_file`.

## Run it (real ag2 agent)

```bash
cp .env.example .env          # then set OPENAI_API_KEY and AGENT_BACKEND=ag2
uv sync --extra ag2           # install ag2[openai,ag-ui] + dishka-ag2
AGENT_BACKEND=ag2 OPENAI_API_KEY=sk-... uv run agent-basic
```

The `ag2` backend mounts AG2's own `AGUIStream` ASGI app at `/run`.

> Note: the `ag2` path uses a **fast-moving beta API** (the AG2 beta framework
> lives under `autogen.beta` in the `ag2` package — there is no separate
> `ag2-beta` distribution). Spots that may need adjusting after `uv sync` are
> marked `VERIFY:` in `backends/ag2_backend.py`.

## Dependency injection

[dishka](https://dishka.readthedocs.io/) provides `AgentConfig` and the
`AgentBackend` as app-scoped singletons (`providers.py`), injected into the
`/run` handler with `FromDishka` (`app.py`). The handler depends only on the
backend *port*, never on a concrete class or `os.environ`.

**`dishka-ag2`** (the dishka↔AG2 integration) does exist on PyPI. It targets the
**`autogen.beta`** agent loop — injecting `FromDishka` dependencies into tools
and prompts via `DishkaAsyncMiddleware`. The AG-UI bridge here wraps a classic
`ConversableAgent` through `AGUIStream`, which does not expose those middleware
hooks, so this example uses **plain dishka** to construct the agent's
collaborators and documents `dishka-ag2` rather than forcing it onto a code path
it does not yet cover. To use `dishka-ag2` fully you would drive the
`autogen.beta` `Agent` directly (composing `DishkaAsyncMiddleware`) and emit
AG-UI events from its event stream — a larger example than this one.

## Lint

```bash
uv run ruff check .
```
