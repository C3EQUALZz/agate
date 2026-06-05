# Configuration

Agate is configured **today** through environment variables read at startup by
`agate-server`. A **file-based `agate.toml`** configuration is being designed in
parallel; the sections below mark clearly what exists now and what is coming.

## Environment variables (today)

These are read once at process start. A missing required variable aborts
startup (fail fast on misconfiguration) rather than starting in a degraded
state.

### Proxy & networking

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `AGENT_ENDPOINT` | **yes** | — | URL of the upstream AG-UI agent that Agate forwards inspected traffic to. |
| `BIND_ADDR` | no | `0.0.0.0:8080` | Address/port Agate listens on for incoming AG-UI traffic. |

### Audit (transparency log)

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `DATABASE_URL` | **yes** | — | PostgreSQL connection string for the Merkle transparency log. Migrations run on startup. |
| `AUDIT_LOG_ID` | no | new log created | UUID of the transparency log to append to. If unset, a fresh log is created and its id is logged so you can pin it on restart. |

### Policy

The policy variables are all optional. With none set, **every tool is permitted
and nothing is redacted**.

| Variable | Format | Meaning |
| --- | --- | --- |
| `POLICY_TOOL_ALLOWLIST` | comma-separated tool names | Only these tools may run. Mutually exclusive with the denylist. |
| `POLICY_TOOL_DENYLIST` | comma-separated tool names | These tools are denied (used only when no allowlist is set). |
| `POLICY_REDACT_PATTERNS` | comma-separated literal markers | Substrings redacted from emitted text before it reaches the client. |

!!! warning "Allowlist and denylist are mutually exclusive"
    Setting both `POLICY_TOOL_ALLOWLIST` and `POLICY_TOOL_DENYLIST` is
    contradictory (which wins?) and **aborts startup** rather than being
    silently resolved. A blank or invalid tool name also aborts startup, so a
    typo cannot silently weaken enforcement.

### Example

```bash
AGENT_ENDPOINT=http://agent:9000
DATABASE_URL=postgres://agate:agate@db:5432/agate
BIND_ADDR=0.0.0.0:8080
AUDIT_LOG_ID=3f6c0b1e-2c9a-4f1e-bb1b-7e2c0b9a1d34
POLICY_TOOL_ALLOWLIST=search,read_file,http_get
POLICY_REDACT_PATTERNS=sk-,AKIA,BEGIN PRIVATE KEY
```

---

## File configuration: `agate.toml` (coming soon)

!!! info "Designed, not yet shipped"
    A TOML configuration file is being designed so deployments can express the
    full configuration in one mounted, version-controllable file instead of a
    growing list of environment variables. The shape below is the **intended
    structure** — keys and defaults are **subject to change** and are marked
    `TODO` where not yet finalized. Environment variables remain supported and
    are expected to **override** file values.

The plan is to mount a single `agate.toml` into the container:

```bash
docker run --rm \
  -v "$PWD/agate.toml:/etc/agate/agate.toml:ro" \
  -e AGATE_CONFIG=/etc/agate/agate.toml \
  agate-server
```

Intended sections (illustrative — **do not rely on these names yet**):

```toml
# agate.toml — INTENDED SHAPE, subject to change (TODO: finalize)

[proxy]
bind_addr      = "0.0.0.0:8080"
agent_endpoint = "http://agent:9000"
# fail_mode    = "closed"   # TODO: fail-open vs fail-closed on policy errors
# limits       = { max_body_bytes = ..., max_run_seconds = ... }  # TODO: DoS budgets

[audit]
database_url = "postgres://agate:agate@db:5432/agate"
# log_id     = "…"          # equivalent of AUDIT_LOG_ID
# hash_algo  = "sha256"     # TODO: Merkle epoch hash (sha2 / sha3 / streebog)
# signing    = { ... }      # TODO: signed tree head key configuration

[policy]
# tools  = { mode = "allowlist", names = ["search", "read_file"] }  # or denylist / allow_all
# redact = ["sk-", "AKIA"]
# TODO: richer rules (regex redaction, per-tool argument constraints, URL screening)

[observability]
# log_level = "info"        # TODO
# tracing   = { ... }       # TODO: OpenTelemetry export

[plugins]
# TODO: toggles for optional adapters/strategies
# ag_ui = true              # the AG-UI transport adapter
# agent_llm = false         # the future agent ↔ LLM adapter
# gost_crypto = false       # Streebog / Kuznyechik / Magma strategies
```

Until `agate.toml` ships, use the environment variables above. This page will be
updated with the authoritative schema, defaults, and precedence rules when the
feature lands.
