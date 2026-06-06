# Installation (Docker)

Agate is designed to run in Docker. The entrypoint is the **`agate-server`**
binary — the composition root that wires the proxy, audit, and policy contexts
together. It is configured by a mounted **`agate.toml`** (see
**[Configuration](configuration.md)**).

## 1. Prerequisites

- A running **AG-UI agent** to put Agate in front of (its URL becomes
  `[proxy].agent_endpoint`).
- A **PostgreSQL** database for the transparency log (its URL becomes
  `[audit].database_url`). Migrations run automatically on startup.

## 2. Get the image

Images are published to GHCR automatically by CI on every push to `main` and on
release tags:

```bash
docker pull ghcr.io/c3equalzz/agate:latest
```

Available tags: `latest` (default branch), `vX.Y.Z` / `vX.Y` (release tags), and
`sha-<commit>` (every build, for pinning an exact commit).

!!! note "Building from source"
    If you prefer to build locally:
    `docker build -t agate -f crates/agate-server/Dockerfile .`

## 3. Write `agate.toml`

Start from [`agate.example.toml`](https://github.com/C3EQUALZz/agate/blob/main/agate.example.toml):

```toml
[proxy]
agent_endpoint = "http://your-agent:9000/run"
bind = "0.0.0.0:8080"

[audit]
database_url = "postgres://agate@db:5432/agate"  # password via env, below

[policy.tools]
mode = "allow-all"
```

## 4. Run it

Mount the file and point `AGATE_CONFIG` at it; pass secrets as `AGATE__*`
environment overrides:

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$PWD/agate.toml:/etc/agate/agate.toml:ro" \
  -e AGATE_CONFIG=/etc/agate/agate.toml \
  -e AGATE__AUDIT__DATABASE_URL='postgres://agate:secret@db:5432/agate' \
  ghcr.io/c3equalzz/agate:latest
```

Point your frontend at `http://localhost:8080` instead of at the agent — Agate
forwards every request to the agent after inspection.

## 5. Pin the transparency log

On first start, Agate creates a fresh transparency log and prints its id:

```text
created transparency log 3f6c…; set AUDIT_LOG_ID=3f6c… to reuse it
```

Set the `AUDIT_LOG_ID` environment variable to that UUID so restarts append to
the **same** log instead of starting a new one (`-e AUDIT_LOG_ID="3f6c…"`).

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
    image: ghcr.io/c3equalzz/agate:latest
    depends_on: [db]
    ports:
      - "8080:8080"
    volumes:
      - ./agate.toml:/etc/agate/agate.toml:ro
    environment:
      AGATE_CONFIG: /etc/agate/agate.toml
      AGATE__AUDIT__DATABASE_URL: "postgres://agate:agate@db:5432/agate"
      # AUDIT_LOG_ID: "…"   # set after first run to reuse the log
```

## Graceful shutdown

On `SIGTERM` (what a container runtime sends to stop) or `SIGINT` (Ctrl+C),
Agate stops accepting new connections, lets in-flight requests finish, then
**flushes the audit outbox** — the records still queued are appended before the
process exits. Safe for rolling restarts and Kubernetes pod termination.

Continue to **[Configuration](configuration.md)** for the full key reference.
