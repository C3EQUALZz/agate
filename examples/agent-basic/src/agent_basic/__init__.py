"""A minimal AG-UI agent that Agate sits in front of.

The package is intentionally small:

- ``ag_ui`` — the AG-UI wire protocol: event constructors and SSE framing,
  matching exactly the JSON shape Agate's proxy inspects.
- ``config`` — typed settings (which backend, LLM model, secret marker).
- ``providers`` — dishka dependency-injection providers.
- ``backends`` — the two agent implementations: ``stub`` (no API key) and
  ``ag2`` (real AutoGen 2 agent).
- ``app`` — the FastAPI application exposing the AG-UI ``POST /run`` endpoint.
"""

__all__ = ["__version__"]

__version__ = "0.1.0"
