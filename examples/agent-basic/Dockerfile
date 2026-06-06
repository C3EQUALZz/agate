# Container image for the agent-basic AG-UI agent (stub mode by default).
# Uses the official uv image so `uv sync` is reproducible from uv.lock.
FROM ghcr.io/astral-sh/uv:python3.12-bookworm-slim

WORKDIR /app

# Install dependencies first (better layer caching). Only the lockfile +
# manifest are needed; the venv is created in /app/.venv.
COPY pyproject.toml uv.lock* README.md ./
COPY src ./src

# --frozen fails if uv.lock is out of date; drop it if you change deps without
# re-locking. Stub mode needs no extras.
RUN uv sync --no-dev --frozen || uv sync --no-dev

ENV AGENT_BACKEND=stub \
    AGENT_HOST=0.0.0.0 \
    AGENT_PORT=8000

EXPOSE 8000

CMD ["uv", "run", "--no-dev", "agent-basic"]
