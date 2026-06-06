# Configuration

`agate-server` (the Docker entrypoint) is configured by a single **`agate.toml`**
file, layered with environment overrides. Mount the file into the container and
point `AGATE_CONFIG` at it. A ready-to-edit template lives at
[`agate.example.toml`](https://github.com/C3EQUALZz/agate/blob/main/agate.example.toml).

## Sources and precedence

Lowest to highest — a later layer overrides an earlier one:

1. **Built-in defaults.**
2. **`agate.toml`** — path from `AGATE_CONFIG` (default `/etc/agate/agate.toml`).
   A missing file is fine; the defaults apply.
3. **Environment** — `AGATE__SECTION__KEY` (uppercase, `__` between levels).
   Prefer env for secrets.

So `[audit].database_url` is overridden by `AGATE__AUDIT__DATABASE_URL`.

```bash
docker run --rm \
  -v "$PWD/agate.toml:/etc/agate/agate.toml:ro" \
  -e AGATE_CONFIG=/etc/agate/agate.toml \
  -e AGATE__AUDIT__DATABASE_URL='postgres://agate:secret@db:5432/agate' \
  ghcr.io/c3equalzz/agate
```

A missing **required** value (`proxy.agent_endpoint`, `audit.database_url`)
aborts startup — fail fast on misconfiguration rather than running degraded.

## `[proxy]`

| Key | Required | Default | Meaning |
| --- | --- | --- | --- |
| `agent_endpoint` | **yes** | — | URL of the upstream AG-UI agent that Agate forwards inspected traffic to. |
| `bind` | no | `0.0.0.0:8080` | Address/port Agate listens on for incoming AG-UI traffic. |

## `[audit]`

| Key | Required | Default | Meaning |
| --- | --- | --- | --- |
| `database_url` | **yes** | — | PostgreSQL connection string for the Merkle transparency log. Migrations run on startup. Prefer `AGATE__AUDIT__DATABASE_URL` for the password. |

The transparency log to append to is pinned by the **`AUDIT_LOG_ID`** environment
variable (a UUID). If unset, a fresh log is created on startup and its id is
printed so you can pin it on the next run.

## `[policy.tools]` and `[policy]`

All policy keys are optional. With none set, **every tool is permitted and
nothing is redacted**.

| Key | Format | Meaning |
| --- | --- | --- |
| `[policy.tools].mode` | `allow-all` \| `allowlist` \| `denylist` | How tool calls are authorized. Default `allow-all`. |
| `[policy.tools].names` | array of tool names | Tools governed by `mode` (ignored when `allow-all`). |
| `[policy].redact` | array of literal markers | Substrings masked (case-insensitive) in emitted text before it reaches the client. |

!!! warning "Invalid policy aborts startup"
    A blank or invalid tool name, or an empty redaction pattern, **aborts
    startup** — a typo must never silently weaken enforcement.

## `[observability.logging]`

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `true` | Install a log subscriber at all; `false` silences logs. |
| `format` | `pretty` | `pretty` (console) or `json` (one object per line, for log shippers). |
| `level` | `info` | Filter directive (e.g. `agate_proxy=debug,info`). `RUST_LOG` overrides it when set. |

At `info` you see lifecycle events: startup, each proxied run, policy denials and
redactions, and transparency-log creation. Raise to `debug` (e.g.
`level = "agate_proxy=debug,info"`) for per-event detail (each forwarded/buffered
event, every appended audit record).

!!! info "Metrics and tracing"
    Prometheus metrics and OpenTelemetry tracing plug into the same
    `[observability]` section and are documented here as they land.

## Full example

```toml
[proxy]
agent_endpoint = "http://agent:8000/run"
bind = "0.0.0.0:8080"

[audit]
# Prefer AGATE__AUDIT__DATABASE_URL for the password.
database_url = "postgres://agate@postgres:5432/agate"

[policy.tools]
mode = "allowlist"
names = ["search", "fetch"]

[policy]
redact = ["sk-", "AKIA"]

[observability.logging]
enabled = true
format = "pretty"
level = "info"
```
