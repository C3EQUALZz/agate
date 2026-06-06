# protected-demo — see Agate protect an AG-UI agent

A `docker-compose` that runs three services and a client script that shows
Agate's protections in action:

```
client ──POST /──▶ Agate (:8080) ──forward──▶ agent (:8000/run)   [agent-basic, stub]
                      │ inspect each SSE event (allow / deny / redact)
                      └──────────▶ Postgres (transparency log)
client ◀──inspected SSE stream──  Agate
```

- **agent** — the [`agent-basic`](../agent-basic/) AG-UI agent in **stub mode**
  (no API key). It emits assistant text containing a fake `sk-…` secret, an
  allowed `search` tool call, and a dangerous `delete_file` tool call.
- **agate** — `ghcr.io/c3equalzz/agate:latest`, configured by
  [`agate.toml`](agate.toml) to **allowlist only `search`** (so `delete_file` is
  denied) and **redact `sk-`** secret markers.
- **db** — Postgres for Agate's tamper-evident transparency log.

## Run

```bash
cd examples/protected-demo
docker compose up --build          # starts db + agent + agate
```

Watch Agate's logs for a line like
`created transparency log <uuid>; set AUDIT_LOG_ID=<uuid> to reuse it`. To make
restarts append to the **same** log, uncomment `AUDIT_LOG_ID` in
`docker-compose.yml` and paste that UUID.

In another terminal, send a run **through Agate** and print the inspected stream:

```bash
cd examples/protected-demo
uv sync
uv run protected-demo-client
```

## What you should see

```
  RUN_STARTED
  TEXT_MESSAGE_CONTENT delta='You asked: ...'
  TEXT_MESSAGE_CONTENT delta='... credential ...: [REDACTED]. ...'   <- secret redacted by Agate
  TOOL_CALL_START name=search id=...                                  (allowed -> forwarded)
  TOOL_CALL_ARGS  delta='{"query":"..."}'
  RUN_ERROR message='...delete_file...'                               <- run blocked by Agate

What Agate did
  [OK] secret marker redacted to [REDACTED] in assistant text
  [OK] dangerous 'delete_file' tool call NOT forwarded (denied)
  [OK] run terminated with RUN_ERROR after the denied tool call
```

Compare with hitting the **agent directly** (the secret and the `delete_file`
call both come through unprotected):

```bash
# the agent is only reachable inside the compose network by default; expose it
# (add `ports: ["8000:8000"]` to the agent service) to try this:
curl -N -X POST http://localhost:8000/run \
  -H 'content-type: application/json' \
  -d '{"threadId":"t","runId":"r","messages":[{"role":"user","content":"hi"}]}'
```

> Exactly how Agate surfaces a denied tool call (a `RUN_ERROR` and/or a dropped
> frame) and how a redaction renders depend on the running Agate build. The
> client highlights `[REDACTED]`, `delete_file`, and `RUN_ERROR` heuristically;
> if your build differs, the raw events are still printed so you can see what it
> did. **VERIFY against the image you pull.**

## Stopping

```bash
docker compose down -v   # -v also drops the Postgres volume (and the log)
```

## Files

```
protected-demo/
  agate.toml             # the policy: allowlist=["search"], redact=["sk-"]
  docker-compose.yml     # db + agent + agate
  pyproject.toml         # the client (uv project)
  src/protected_demo_client/
    sse.py               # minimal SSE parser
    __main__.py          # POST a run through Agate, annotate the stream
```
