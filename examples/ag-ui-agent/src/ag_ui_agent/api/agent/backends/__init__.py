from ag_ui_agent.api.agent.backends.stub import StubAgUiStreamer

__all__ = ["StubAgUiStreamer"]

# Note: backends.ag2 is intentionally NOT imported here. It imports ``autogen``
# and ``dishka_ag2`` (the optional ``ag2`` extra), so it is imported lazily by
# ``main.providers.agent`` only when ``AGENT__BACKEND=ag2``. Keeping it out of
# this package ``__init__`` lets the stub backend run with the extra uninstalled.
