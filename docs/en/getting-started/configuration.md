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
| `max_concurrent_requests` | no | `256` | Maximum concurrently in-flight proxied runs. Each holds an upstream connection for its full stream; requests over the cap are shed with `503` (not queued), so a flood cannot exhaust memory/connections. |
| `max_response_events` | no | `100000` | Per-run cap on response events streamed to the client. A runaway/hostile agent over this is cut off with a `RUN_ERROR`. `0` = unlimited. |
| `max_response_bytes` | no | `67108864` | Per-run cap on response bytes streamed to the client (64 MiB). `0` = unlimited. |
| `api_key` | no | — | Single API key required on the `X-API-Key` header (a shorthand, merged with `api_keys`). Prefer `AGATE__PROXY__API_KEY` for the secret. |
| `api_keys` | no | `[]` | Accepted API keys; a request matching **any** is authenticated (`401` otherwise). Holding several at once enables zero-downtime **rotation**: add the new key, migrate clients, then drop the old. With `api_key` and `api_keys` both empty, the proxy is open. |

!!! note "Liveness vs readiness probes"
    `/healthz` (liveness) returns `200` whenever the process is up. `/readyz`
    (readiness) returns `200` only when the transparency-log store is
    reachable, else `503` — point your orchestrator's readiness probe at it so
    traffic is held until Agate can record. The reachability check is behind a
    `HealthCheck` port, so it stays correct if the store backend changes. Both
    probes bypass the API-key and body-size guards.

## `[audit]`

| Key | Required | Default | Meaning |
| --- | --- | --- | --- |
| `backend` | no | `postgres` | Which persistence backend assembles at startup. `postgres` today; further backends land behind Cargo features. |
| `database_url` | **yes** | — | PostgreSQL connection string for the Merkle transparency log (for `backend = "postgres"`). Migrations run on startup. Prefer `AGATE__AUDIT__DATABASE_URL` for the password. |
| `max_connections` | no | `10` | Maximum pooled database connections. |
| `acquire_timeout_secs` | no | `30` | How long to wait for a free pooled connection before erroring. |
| `connect_max_retries` | no | `10` | Initial-connect retries before startup gives up (`0` = try once). Rides out a database still starting beside Agate (compose/Kubernetes) instead of crashing on the first failed connect. |
| `connect_backoff_secs` | no | `1` | Base backoff between connect attempts (doubled each retry, capped). |

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
| `[[policy.tools.deny_arguments]]` | tables of `{ tool?, contains }` | Argument-level deny rules: a permitted tool call is **blocked** when its arguments contain `contains` (case-insensitive). `tool` scopes the rule to one tool; omit it for any tool. |
| `[policy].redact` | array of literal markers | Substrings masked (case-insensitive) in emitted text and tool results before they reach the client. |
| `[policy].redact_regex` | array of regex patterns | Regex markers masked in emitted text and tool results (full `regex` syntax; prefix `(?i)` for case-insensitivity). An invalid expression aborts startup. |
| `[policy].fail_mode` | `open` \| `closed` | What to do if a policy decision times out: forward (`open`) or block (`closed`). Default `closed` (safety over availability). |
| `[policy].decision_timeout_ms` | integer (ms) | Deadline for one policy decision. Default `5000`; must be > 0. |
| `[policy].on_malformed_event` | `forward` \| `drop` \| `terminate` | What to do with a recognized response event that is malformed (a known type missing a required field), so it cannot be inspected. `forward` passes the raw frame, `drop` discards it, `terminate` ends the run. Default `terminate` (it must not bypass the policy). |

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
- `agate_upstream_errors_total{kind="connect|timeout|status|stream"}` — upstream agent request/stream failures by kind.
- `agate_audit_records_appended_total` / `agate_audit_records_dropped_total` — transparency-log writes vs. drops (a non-zero drop rate means audit is falling behind — alert on it).

A ready-to-run Prometheus + Grafana stack with a pre-built dashboard lives in
[`deploy/observability/`](https://github.com/C3EQUALZz/agate/tree/main/deploy/observability).

## `[observability.tracing]`

OTLP trace export — the third observability pillar beside logs and metrics. When
tracing is off, spans are still created but not exported; they are visible in
logs only when `[observability.logging].enabled = true` (the subscriber that
both renders spans to logs and exports them is installed only then).

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `false` | Export spans to an OTLP collector over gRPC. |
| `endpoint` | `http://localhost:4317` | OTLP gRPC endpoint of the collector. |
| `service_name` | `agate-server` | `service.name` reported on exported spans. |

Spans cover the request path end to end:

- `proxy_run` — one per proxied run on the data plane.
- `audit.request` — one per dispatched audit command/query. A `TracingBehavior`
  wraps the whole mediator pipeline (outermost, enclosing the metrics and
  transaction behaviors), so every use case is traced uniformly — new use cases
  get a span for free.
- `db.log.load` / `db.log.save` / `db.proof.inclusion` / `db.proof.consistency`
  — one per SQL statement, nested under the `audit.request` span that issued it.

When tracing is enabled (and `[observability.logging].enabled = true`, so the
subscriber is installed), spans are flushed on graceful shutdown. Point
`endpoint` at an OpenTelemetry Collector (or any OTLP/gRPC backend) to collect
per-run traces.

## `[tls]`

Terminate TLS at Agate's own listener. Off by default — Agate then serves plain
HTTP, which is only sensible behind a TLS-terminating gateway (an ingress, load
balancer, or service mesh). Enable it to serve HTTPS directly (e.g. for a
zero-trust deployment with no separate terminator).

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `false` | Serve HTTPS instead of plain HTTP. When `false`, `cert`/`key` are ignored. |
| `cert` | — | Path to the PEM certificate chain (leaf certificate first). Required when enabled. |
| `key` | — | Path to the PEM private key for `cert`. Required when enabled. |

When enabled, a missing or invalid `cert`/`key` **aborts startup** (fail fast on
a misconfigured listener). Both probes and the data plane are then served over
the same TLS listener on `proxy.bind`.

## Full example

```toml
[proxy]
agent_endpoint = "http://agent:8000/run"
bind = "0.0.0.0:8080"
connect_timeout_secs = 5
read_timeout_secs = 60
max_body_bytes = 1048576
max_concurrent_requests = 256
max_response_events = 100000
max_response_bytes = 67108864
# api_key = "change-me"          # single key; prefer AGATE__PROXY__API_KEY
# api_keys = ["current", "next"] # multiple keys for zero-downtime rotation

[audit]
backend = "postgres"
# Prefer AGATE__AUDIT__DATABASE_URL for the password.
database_url = "postgres://agate@postgres:5432/agate"
max_connections = 10
acquire_timeout_secs = 30
connect_max_retries = 10
connect_backoff_secs = 1

[policy.tools]
mode = "allowlist"
names = ["search", "fetch"]

[[policy.tools.deny_arguments]]
tool = "search"
contains = "rm -rf"
[[policy.tools.deny_arguments]]
contains = "AKIA"

[policy]
redact = ["sk-", "AKIA"]
redact_regex = ["sk-[A-Za-z0-9]{20,}", "AKIA[0-9A-Z]{16}"]
fail_mode = "closed"
decision_timeout_ms = 5000
on_malformed_event = "terminate"

[observability.logging]
enabled = true
format = "pretty"
level = "info"

[observability.metrics]
enabled = true
exporter = "prometheus"
bind = "0.0.0.0:9090"

[observability.tracing]
enabled = false
endpoint = "http://localhost:4317"
service_name = "agate-server"

[tls]
enabled = false
cert = "/etc/agate/tls/cert.pem"
key = "/etc/agate/tls/key.pem"
```
