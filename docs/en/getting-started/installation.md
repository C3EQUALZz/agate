# Installation (Docker)

Agate is designed to run in Docker. The entrypoint is the **`agate-server`**
binary — the composition root that wires the proxy, audit, and policy contexts
together.

!!! warning "Image not yet published"
    A prebuilt image is not yet published to a registry. Until then, build the
    image from the repository. The image name `agate-server` below is a
    placeholder for whatever you tag your build with.

## 1. Prerequisites

- A running **AG-UI agent** to put Agate in front of (its URL becomes
  `AGENT_ENDPOINT`).
- A **PostgreSQL** database for the transparency log (its URL becomes
  `DATABASE_URL`). Migrations run automatically on startup.

## 2. Build the image

```bash
# from the repository root
docker build -t agate-server -f crates/agate-server/Dockerfile .
```

!!! note
    If the crate does not yet ship a `Dockerfile`, build the binary with
    `cargo build --release -p agate-server` and run it directly with the same
    environment variables described below. A `Dockerfile` is on the roadmap.

## 3. Run it

```bash
docker run --rm \
  -p 8080:8080 \
  -e AGENT_ENDPOINT="http://your-agent:9000" \
  -e DATABASE_URL="postgres://agate:agate@db:5432/agate" \
  -e BIND_ADDR="0.0.0.0:8080" \
  agate-server
```

Point your frontend at `http://localhost:8080` instead of at the agent. Agate
forwards every request to `AGENT_ENDPOINT` after inspection.

## 4. Pin the transparency log

On first start, Agate creates a fresh transparency log and prints its id:

```text
created transparency log 3f6c…; set AUDIT_LOG_ID=3f6c… to reuse it
```

Set `AUDIT_LOG_ID` to that UUID so restarts append to the **same** log instead
of starting a new one:

```bash
docker run --rm \
  -e AUDIT_LOG_ID="3f6c0b1e-…" \
  ... \
  agate-server
```

## 5. Apply a policy (optional)

By default Agate permits every tool and redacts nothing. Tighten it with the
`POLICY_*` variables (see **[Configuration](configuration.md)**):

```bash
docker run --rm \
  -e POLICY_TOOL_ALLOWLIST="search,read_file" \
  -e POLICY_REDACT_PATTERNS="sk-,AKIA" \
  ... \
  agate-server
```

## Example: Docker Compose

```yaml
services:
  db:
    image: postgres:17
    environment:
      POSTGRES_USER: agate
      POSTGRES_PASSWORD: agate
      POSTGRES_DB: agate

  agate:
    image: agate-server # built locally for now
    depends_on: [db]
    ports:
      - "8080:8080"
    environment:
      AGENT_ENDPOINT: "http://your-agent:9000"
      DATABASE_URL: "postgres://agate:agate@db:5432/agate"
      BIND_ADDR: "0.0.0.0:8080"
      # AUDIT_LOG_ID: "…"           # set after first run to reuse the log
      # POLICY_TOOL_ALLOWLIST: "…"  # see Configuration
```

Continue to **[Configuration](configuration.md)** for the full variable
reference.
