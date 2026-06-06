# Multi-stage build for the AG-UI agent.
#
# By default this builds the STUB backend (no API key, no AG2/OpenAI deps), so
# the protected demo runs fully offline. To build the real-AG2 image instead,
# pass --build-arg EXTRAS="--extra ag2" and run with AGENT__BACKEND=ag2 plus an
# AGENT__OPENAI_API_KEY.
FROM python:3.13-slim-bookworm AS base
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/
WORKDIR /app
ENV UV_LINK_MODE=copy UV_COMPILE_BYTECODE=1

ARG EXTRAS=""

FROM base AS deps
ARG EXTRAS
COPY pyproject.toml uv.lock* README.md ./
RUN uv sync --no-install-project --no-dev ${EXTRAS}

FROM base AS runtime
ARG EXTRAS
COPY --from=deps /app/.venv /app/.venv
COPY pyproject.toml uv.lock* README.md ./
COPY src/ ./src
RUN uv sync --no-dev ${EXTRAS}

RUN groupadd -r app && useradd -r -g app -d /app app && chown -R app:app /app
USER app

ENV PATH="/app/.venv/bin:$PATH" PYTHONUNBUFFERED=1
EXPOSE 8000
HEALTHCHECK --interval=15s --timeout=3s --start-period=10s --retries=5 \
  CMD python -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8000/api/health').read()" || exit 1
CMD ["uvicorn", "ag_ui_agent.main.entrypoint:app", "--host", "0.0.0.0", "--port", "8000"]
