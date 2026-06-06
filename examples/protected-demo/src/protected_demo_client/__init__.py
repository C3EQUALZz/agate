"""Client that drives an AG-UI run *through Agate* and prints what comes back.

The point is to make Agate's protections visible: compared with hitting the
agent directly, the stream through Agate has the secret redacted and the
dangerous tool call denied.
"""

__all__ = ["__version__"]

__version__ = "0.1.0"
