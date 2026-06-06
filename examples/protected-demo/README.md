# protected-demo

Put the [`ag-ui-agent`](../ag-ui-agent) **behind Agate** and watch the proxy
enforce a policy on a live AG-UI run. A `docker-compose` brings up three
services and a small layered client posts one run through Agate:

```
client ──POST /──▶ Agate (:8080) ──forward──▶ agent (:8000/api/run)
                      │ inspect each SSE event (allow / deny / redact)
                      └──────────▶ Postgres transparency log
```

| Service | Image | Role |
| --- | --- | --- |
| `agent` | built from `../ag-ui-agent` (stub backend) | the AG-UI upstream |
| `agate` | `ghcr.io/c3equalzz/agate:latest` | the inline reverse proxy |
| `db` | `postgres:17-alpine` | the transparency log |

## The policy ([`agate.toml`](agate.toml))

```toml
[proxy]
agent_endpoint = "http://agent:8000/api/run"   # where Agate forwards runs

[policy.tools]
mode  = "allowlist"
names = ["search_documents"]                    # only this tool may run

[policy]
redact = ["sk-", "AKIA"]                         # mask these markers in text
```

So Agate will: **allow** `search_documents`, **deny** `delete_file` (not on the
allowlist — surfaced to the client as `RUN_ERROR`), and **redact** the agent's
leaked `sk-...` token before the client sees it.

## Run it

```bash
# 1) Bring up agent + Agate + Postgres (first run builds the agent image).
docker compose up --build

# 2) In another terminal, from this directory, post a run THROUGH Agate:
uv run protected-demo-client
# or: uv run protected-demo-client --url http://localhost:8080/ --prompt "clean up old files"
```

## What you'll see

The client prints each inspected AG-UI event, then a summary. With the demo
policy you should observe:

```
What Agate did
  [OK] secret marker redacted to [REDACTED] in assistant text
  [OK] dangerous 'delete_file' tool call NOT forwarded (denied)
  [OK] run terminated with RUN_ERROR after the denied tool call
  [OK] allowed 'search_documents' tool call passed through
```

- The `search_documents` tool call (safe, allowlisted) passes through unchanged.
- The `delete_file` tool call (dangerous) **never reaches the client** — Agate
  denies it and the run ends in `RUN_ERROR`.
- The assistant text shows `[REDACTED]` where the agent had emitted `sk-...`.

Every `(event, verdict)` decision was appended to Agate's transparency log in
Postgres — see [`../audit-verify`](../audit-verify) to inspect it.

## The client is layered too

Even though it is just a demo client, it keeps clean boundaries (src-layout):

```
src/protected_demo_client/
├── config.py                 # ClientConfig (url / prompt / timeout)
├── transport/
│   ├── sse.py                # SSE frame parser (matches Agate's framing)
│   └── agate.py              # POST a run to Agate, stream its events
├── domain/observation.py     # infer Agate's verdicts from the observable stream
├── render.py                 # terminal presentation (pure, no I/O but print)
└── cli.py                    # argument parsing + orchestration
```

`uv run pytest` runs the parser + observation unit tests (no Docker needed).

## Tuning the demo

- Change `names` in `agate.toml` to `["search_documents", "delete_file"]` and
  re-run: the dangerous call now passes (the summary line flips to `--`).
- Remove `"sk-"` from `redact` and re-run: the secret is no longer masked.
- Point `--url` at the agent directly (`http://localhost:8000/api/run`, if you
  expose it) to see the **unprotected** stream for contrast.
