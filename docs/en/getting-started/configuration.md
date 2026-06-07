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
| `connect_timeout_secs` | no | `5` | Fail-fast connect timeout to the upstream agent. |
| `read_timeout_secs` | no | `60` | Idle timeout between upstream SSE chunks. **Not** an overall deadline — a healthy stream runs on. |
| `max_body_bytes` | no | `1048576` | Maximum accepted request body size (1 MiB). Oversized requests get `413`. |
| `api_key` | no | — | If set, required on the `X-API-Key` header (else `401`). Blank/absent leaves the proxy open. Prefer `AGATE__PROXY__API_KEY` for the secret. |

!!! note "Liveness vs readiness probes"
    `/healthz` (liveness) returns `200` whenever the process is up. `/readyz`
    (readiness) returns `200` only when the transparency-log database is
    reachable, else `503` — point your orchestrator's readiness probe at it so
    traffic is held until Agate can record. Both probes bypass the API-key and
    body-size guards.

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
| `[policy].fail_mode` | `open` \| `closed` | What to do if a policy decision times out: forward (`open`) or block (`closed`). Default `closed` (safety over availability). |
| `[policy].decision_timeout_ms` | integer (ms) | Deadline for one policy decision. Default `5000`; must be > 0. |

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

## `[observability.metrics]`

A Prometheus scrape endpoint on **its own port**, kept off the public data-plane
port (scrape it from inside the network).

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `false` | Install the metrics recorder + exporter. When off, metrics are no-ops. |
| `exporter` | `prometheus` | `prometheus` (a `/metrics` endpoint) or `none`. |
| `bind` | `0.0.0.0:9090` | Address the `/metrics` endpoint listens on. |

Exposed metrics:

- `agate_runs_total` — runs proxied.
- `agate_events_inspected_total{outcome="forward|buffer|transform|deny|terminate"}` — inspected events by outcome (the verdict breakdown).
- `agate_upstream_errors_total` — upstream agent request/stream failures.
- `agate_audit_records_appended_total` / `agate_audit_records_dropped_total` — transparency-log writes vs. drops (a non-zero drop rate means audit is falling behind — alert on it).

A ready-to-run Prometheus + Grafana stack with a pre-built dashboard lives in
[`deploy/observability/`](https://github.com/C3EQUALZz/agate/tree/main/deploy/observability).

!!! info "Distributed tracing"
    OpenTelemetry (OTLP) tracing plugs into the same `[observability]` section
    and is documented here as it lands.

## Full example

```toml
[proxy]
agent_endpoint = "http://agent:8000/run"
bind = "0.0.0.0:8080"
connect_timeout_secs = 5
read_timeout_secs = 60
max_body_bytes = 1048576
# api_key = "change-me"   # prefer AGATE__PROXY__API_KEY

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

[observability.metrics]
enabled = true
exporter = "prometheus"
bind = "0.0.0.0:9090"
```
