"""Utility agent tools (a safe, read-only status probe)."""

from autogen.beta import tool


@tool
def echo_status(detail: str = "") -> str:
    """Return a short status line (a no-dependency demo tool).

    Args:
        detail: optional text to include in the status line.
    """
    suffix = f": {detail}" if detail else ""
    return f"workspace assistant ready{suffix}"
