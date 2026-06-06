"""Entrypoint: ``uv run agent-basic`` (or ``python -m agent_basic``).

Starts uvicorn serving the AG-UI app on ``0.0.0.0:8000`` (host/port overridable
via ``AGENT_HOST`` / ``AGENT_PORT``). Agate forwards to ``/run`` on this server.
"""

from __future__ import annotations

import os


def main() -> None:
    import uvicorn

    host = os.getenv("AGENT_HOST", "0.0.0.0")
    port = int(os.getenv("AGENT_PORT", "8000"))
    # Pass the import string so uvicorn can manage the app lifecycle.
    uvicorn.run("agent_basic.app:app", host=host, port=port, log_level="info")


if __name__ == "__main__":
    main()
