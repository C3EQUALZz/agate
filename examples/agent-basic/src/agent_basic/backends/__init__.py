"""Agent backends: pluggable implementations behind one interface.

``AgentBackend`` is the port the FastAPI handler depends on. Two adapters:

- ``StubBackend`` — scripted AG-UI events; no API key, fully deterministic.
- ``Ag2Backend`` — a real AutoGen 2 (``ag2``) agent bridged to AG-UI.

dishka picks which one to provide based on ``AgentConfig.backend``.
"""

from __future__ import annotations

from .base import AgentBackend
from .stub import StubBackend

__all__ = ["AgentBackend", "StubBackend"]
