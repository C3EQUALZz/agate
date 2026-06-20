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
| `rate_limit_per_second` | no | `0` | Sustained per-client-IP request rate. A source IP over budget is shed with `429 Too Many Requests` + a `Retry-After` hint. `0` = disabled. The IP is the connection peer, so enable only where Agate sees the real client (behind a load balancer it is the balancer's IP). |
| `rate_limit_burst` | no | `0` | Burst depth for the per-IP limit — the largest instantaneous burst before the sustained rate applies. `0` falls back to `rate_limit_per_second`. |
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
| `checkpoint_interval_secs` | no | `0` | How often a background task issues a signed checkpoint (STH) for the log, in seconds (`0` = disabled). The log's own tamper-evidence cadence (like a CT log's STH frequency); an idle log between ticks is signed but not re-anchored. Requires a signing key (see below). |
| `checkpoint_key_id` | no | `checkpoint-ed25519` | The signing key id the periodic issuer requests; must match the key the store loaded (`AUDIT_CHECKPOINT_KEY_ID`). |
| `outbox_capacity` | no | `1024` | How many inspected records may queue for the transparency log before the outbox is full. Bounded so a slow database cannot grow memory without limit. |
| `outbox_on_full` | no | `block` | What the proxy does when the outbox is full: `block` (apply backpressure — a slow DB slows the proxy, never loses a record; the default) or `shed` (drop the record, loudly logged and counted, so the proxy keeps serving at the cost of a transparency-log gap). |

### Checkpoint signing key

The Ed25519 key is supplied **only** through the environment, never the config file:

- **`AUDIT_CHECKPOINT_SEED`** — 32-byte seed as 64 hex chars. Without it, checkpoint signing is disabled (a periodic interval then has nothing to sign).
- **`AUDIT_CHECKPOINT_KEY_ID`** — optional key id (default `checkpoint-ed25519`); keep it equal to `[audit].checkpoint_key_id`.

The transparency log to append to is pinned by the **`AUDIT_LOG_ID`** environment
variable (a UUID). If unset, a fresh log is created on startup and its id is
printed so you can pin it on the next run.

## `[policy.tools]` and `[policy]`

All policy keys are optional. With none set, **every tool is permitted and
nothing is redacted**.

| Key | Format | Meaning |
| --- | --- | --- |
| `[policy.tools].mode` | `allow-all` \| `allowlist` \| `denylist` | How tool calls are authorized. Default `allow-all`. |
| `[policy.tools].names` | array of tool matchers | Tools governed by `mode` (ignored when `allow-all`). Each entry is matched against the **whole** tool name, case-sensitively, by kind: a bare name is **exact** (`search`); `glob:` is shell-style `*`/`?` (`glob:fs.*`); `regex:` is a regex anchored to the whole name (`regex:db_.*`). So `search` never matches `research`, and `glob:fs.*` covers every `fs.` tool. An invalid glob/regex aborts startup. |
| `[[policy.tools.deny_arguments]]` | tables of `{ tool?, path?, contains \| matches }` | Argument-level deny rules: a permitted tool call is **blocked** when its arguments match. Each rule sets exactly one of `contains` (case-insensitive literal) or `matches` (regex). `tool` scopes the rule to one tool; omit it for any tool. `path` (a dotted path like `url` or `config.endpoint`) scopes the match to one field of the parsed arguments, so it cannot fire on an unrelated field carrying the same text; omit it to match the whole raw argument string. A path rule does not fire when the arguments are not valid JSON or the path is absent. |
| `[[policy.tools.deny_results]]` | tables of `{ tool?, path?, contains \| matches }` | Result-level deny rules: a tool **result** is **blocked** (dropped before the client) when its content matches — the result-side counterpart to `deny_arguments`. Same `tool` / `path` / `contains` / `matches` shape. `tool` only fires when the result's tool is known (correlated from the call's start) and matches. |
| `[policy].redact` | array of literal markers | Substrings masked (case-insensitive) in emitted text and tool results before they reach the client. |
| `[policy].redact_regex` | array of regex patterns | Regex markers masked in emitted text and tool results (full `regex` syntax; prefix `(?i)` for case-insensitivity). An invalid expression aborts startup. |
| `[policy].fail_mode` | `open` \| `closed` | What to do if a policy decision times out: forward (`open`) or block (`closed`). Default `closed` (safety over availability). |
| `[policy].decision_timeout_ms` | integer (ms) | Deadline for one policy decision. Default `5000`; must be > 0. |
| `[policy].on_malformed_event` | `forward` \| `drop` \| `terminate` | What to do with a recognized response event that is malformed (a known type missing a required field), so it cannot be inspected. `forward` passes the raw frame, `drop` discards it, `terminate` ends the run. Default `terminate` (it must not bypass the policy). |
| `[policy.session_memory].enabled` | bool | Cross-run replay memory: once a tool is denied in a run, refuse it (by name) for the rest of the session, so the agent cannot retry it with varied arguments in a later run. Defense-in-depth over the stateless policy. Default `false`. |
| `[policy.session_memory].ttl_secs` | integer (s) | How long a session's quarantine survives without activity. A session idle longer than this is forgotten. Default `3600`; must be > 0 when enabled. |
| `[policy.session_memory].backend` | `memory` \| `redis` | Where the ledger lives. `memory` is process-local (lost on restart, not shared across replicas); `redis` is shared across replicas and restarts. Default `memory`. The Redis backend fails open — if Redis is unreachable it degrades to no memory, never a wrong allow. |
| `[policy.session_memory].redis_url` | string | Redis connection URL (e.g. `redis://127.0.0.1:6379`). **Required** when `backend = "redis"`, ignored otherwise. |
| `[policy].backend` | `ruleset` \| `cel` | Which engine decides verdicts. `ruleset` (default) is the built-in static policy documented above. `cel` hands every decision to operator-authored CEL rules from `[policy.cel]` — the static rules above are then **not** consulted. Selecting `cel` requires a build with the `policy-cel` Cargo feature. |
| `[policy.cel].policy_path` | string | Path to the CEL policy file (a TOML list of `[[rule]]` entries; see below). **Required** when `backend = "cel"`. Every rule is compiled at startup, so a parse error **aborts the process**. |

!!! warning "Invalid policy aborts startup"
    A blank or invalid tool name, or an empty redaction pattern, **aborts
    startup** — a typo must never silently weaken enforcement.

### The CEL policy engine (`backend = "cel"`)

The static ruleset above covers the common cases declaratively. For policies that
need expressions — comparisons, boolean logic, addressing nested fields — Agate
ships an alternative engine that evaluates [CEL][cel] (Common Expression
Language) rules. CEL is **non-Turing-complete** (no loops or recursion), so every
expression terminates; the `decision_timeout_ms` guard above still bounds a
decision as a backstop. It is a separate `PolicyPort` backend
selected with `[policy].backend = "cel"`, available only in a build with the
`policy-cel` Cargo feature (`cargo build -p agate-server --features policy-cel`).

The policy file is a TOML list of `[[rule]]` tables, evaluated **in order**; the
**first** rule whose `when` is `true` wins. If **no** rule matches, the event is
**allowed** — the rules enumerate what is blocked or redacted, so add a trailing
catch-all `when = "true"` deny rule for a default-deny posture.

| Field | Required | Meaning |
| --- | --- | --- |
| `when` | yes | A CEL **boolean** expression over `action` and `context` (below). |
| `effect` | yes | `deny` (block with `reason`), `redact` (replace the event text), or `allow` (pass and stop evaluating). |
| `reason` | no | Deny message (for `effect = "deny"`). Defaults to a generic reason. |
| `replacement` | no | A CEL **string** expression producing the replacement text (for `effect = "redact"`). It applies only to messages and tool results; a redact rule matching any other event kind **fails closed** (the event is denied, not passed through). The expression sees the full `action`, so do **not** echo the matched secret back (`replacement = 'action.text'` would mask it to itself). If it errors or yields a non-string, it falls back to `"[REDACTED]"` (logged). Defaults to `"[REDACTED]"`. |

Each rule sees two variables. `action` is a flat map describing the event — every
key is always present (`null` when not applicable), so a rule may name any field
without erroring on a missing key:

| `action` field | Present for | Value |
| --- | --- | --- |
| `kind` | every event | `"tool_call"`, `"message"`, `"tool_result"`, `"state"`, or `"other"`. |
| `name` | tool calls, tool results | The tool name. |
| `arguments` | tool calls | The raw argument string. |
| `arguments_json` | tool calls | The arguments **parsed** as JSON (address fields: `action.arguments_json.url`), or `null` if they are not valid JSON. |
| `text` | messages | The emitted message text. |
| `content` | tool results | The raw result content. |
| `content_json` | tool results | The result **parsed** as JSON, or `null`. |
| `state_json` | state mutations | The state payload **parsed** as JSON, or `null`. |

`context` carries the run identity: `context.session_id` and `context.run_id`
(both strings).

```toml
# cel-policy.toml — referenced by [policy.cel].policy_path

# Block a tool by name.
[[rule]]
when = 'action.kind == "tool_call" && action.name == "delete_file"'
effect = "deny"
reason = "destructive tool is not permitted"

# Block an SSRF-shaped argument by addressing a parsed field. Guard the nullable
# field first: without `!= null`, a non-JSON argument makes the rule error and be
# skipped (see the null-guard note below) rather than block.
[[rule]]
when = 'action.arguments_json != null && action.arguments_json.url.startsWith("http://169.254.169.254")'
effect = "deny"
reason = "link-local metadata endpoint"

# Redact an API key shape in emitted message text.
[[rule]]
when = 'action.kind == "message" && action.text.contains("sk-")'
effect = "redact"
replacement = '"[REDACTED]"'

# Default-deny: anything not explicitly allowed above is blocked.
[[rule]]
when = "true"
effect = "deny"
reason = "not permitted by policy"
```

[cel]: https://cel.dev/

!!! warning "Guard nullable fields — an erroring rule does not block"
    A rule that **errors** while evaluating (for example, reaching into
    `action.arguments_json.url` when the arguments are not JSON, so
    `arguments_json` is `null`) is treated as *not matched* — logged, then
    skipped — never a hard failure. It therefore does **not** block on its own:
    if no later rule matches, the event is allowed. Guard nullable fields
    (`action.arguments_json != null && action.arguments_json.url...`) and, where
    you need default-deny, end the file with a `when = "true"` deny rule.

#### Hot-reload (`SIGHUP`)

Send the process **`SIGHUP`** to re-read and recompile the policy file without a
restart (`kill -HUP <pid>`, or `docker kill --signal=HUP <container>`). The new
rule set is swapped in atomically — a decision already in flight keeps the rules
it started with, and there is no lock on the request path.

The reload is **fail-safe**: if the file is missing, unparsable, or any rule
fails to compile, the running policy is **kept** (the reload error is logged and
the previous, known-good rules stay in force) — a bad edit never leaves the
gateway without a policy. A reload that produces **zero** rules (an empty or
truncated file — e.g. a non-atomic write caught mid-flight) is likewise refused,
since no rules means allow-all; prefer an atomic write (write a temp file, then
rename) when editing live. Hot-reload is Unix-only; on other platforms the policy
is fixed at startup.

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
- `agate_audit_outbox_depth` / `agate_audit_outbox_capacity` — gauges of how full the audit outbox is; depth approaching capacity is backpressure building (the proxy is about to slow down under `block`, or shed under `shed`). Alert before it saturates.

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
rate_limit_per_second = 0        # per-client-IP request cap; 0 = disabled
rate_limit_burst = 0             # burst depth; 0 falls back to the rate
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
checkpoint_interval_secs = 0   # 0 = disabled; set with AUDIT_CHECKPOINT_SEED
checkpoint_key_id = "checkpoint-ed25519"
outbox_capacity = 1024
outbox_on_full = "block"   # block (backpressure) | shed (drop + alert)

[policy.tools]
mode = "allowlist"
names = ["search", "fetch", "glob:fs.*", "regex:db_.*"]

[[policy.tools.deny_arguments]]
tool = "search"
contains = "rm -rf"
[[policy.tools.deny_arguments]]
tool = "fetch"
path = "url"
matches = "^https?://169\\.254\\.169\\.254"
[[policy.tools.deny_results]]
contains = "BEGIN RSA PRIVATE KEY"

[policy]
backend = "ruleset"              # or "cel" (needs the policy-cel build); see [policy.cel]
redact = ["sk-", "AKIA"]
redact_regex = ["sk-[A-Za-z0-9]{20,}", "AKIA[0-9A-Z]{16}"]
fail_mode = "closed"
decision_timeout_ms = 5000
on_malformed_event = "terminate"

# [policy.cel]
# policy_path = "/etc/agate/cel-policy.toml"   # required when backend = "cel"

[policy.session_memory]
enabled = false
ttl_secs = 3600
backend = "memory"
# redis_url = "redis://127.0.0.1:6379"  # required when backend = "redis"

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
