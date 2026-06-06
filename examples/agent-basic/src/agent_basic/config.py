"""Typed configuration for the agent, read from environment variables.

dishka provides this object to the rest of the app, so swapping how config is
sourced (env, file, secrets manager) is a one-line change in ``providers.py``.
"""

from __future__ import annotations

import os
from dataclasses import dataclass

# The default secret the stub agent tries to leak, so the protected-demo can
# show Agate redacting it. ``sk-`` is also a realistic OpenAI-key prefix.
DEFAULT_SECRET = "sk-EXAMPLE-DEMO-SECRET-1234567890"


@dataclass(frozen=True, slots=True)
class AgentConfig:
    """Settings that select and shape the agent backend."""

    # "stub" (scripted, no API key) or "ag2" (real AutoGen 2 agent).
    backend: str = "stub"
    # LLM model for the ag2 backend.
    model: str = "gpt-4o-mini"
    # OpenAI API key for the ag2 backend; unused by the stub.
    openai_api_key: str | None = None
    # A secret the stub emits in its reply, to demonstrate Agate's redaction.
    demo_secret: str = DEFAULT_SECRET

    @classmethod
    def from_env(cls) -> AgentConfig:
        """Build config from ``AGENT_*`` / ``OPENAI_*`` environment variables."""
        return cls(
            backend=os.getenv("AGENT_BACKEND", "stub").strip().lower(),
            model=os.getenv("AGENT_MODEL", "gpt-4o-mini"),
            openai_api_key=os.getenv("OPENAI_API_KEY"),
            demo_secret=os.getenv("AGENT_DEMO_SECRET", DEFAULT_SECRET),
        )
