# Multi-stage build for the AG-UI agent.
#
# The agent runs the real autogen.beta (AG2) backend, so the container needs an
# OpenAI key at runtime (AGENT__OPENAI_API_KEY). There is no offline mode.
FROM python:3.13-slim-bookworm AS base
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/
WORKDIR /app
ENV UV_LINK_MODE=copy UV_COMPILE_BYTECODE=1

FROM base AS deps
COPY pyproject.toml uv.lock* README.md ./
RUN uv sync --no-install-project --no-dev

FROM base AS runtime
COPY --from=deps /app/.venv /app/.venv
COPY pyproject.toml uv.lock* README.md ./
COPY src/ ./src
RUN uv sync --no-dev

RUN groupadd -r app && useradd -r -g app -d /app app && chown -R app:app /app
USER app

ENV PATH="/app/.venv/bin:$PATH" PYTHONUNBUFFERED=1
EXPOSE 8000
HEALTHCHECK --interval=15s --timeout=3s --start-period=10s --retries=5 \
  CMD python -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8000/api/health').read()" || exit 1
CMD ["uvicorn", "--factory", "ag_ui_agent.main.entrypoint:create_app", "--host", "0.0.0.0", "--port", "8000"]
