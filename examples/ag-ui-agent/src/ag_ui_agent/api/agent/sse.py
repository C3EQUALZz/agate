"""Tiny AG-UI SSE encoder used by the stub backend.

Emits exactly the frame shape Agate parses and re-emits: ``data: {json}\\n\\n``,
one JSON event object per frame. Event ``type`` values match the discriminators
Agate inspects (``RUN_STARTED``, ``TOOL_CALL_START``, ``TEXT_MESSAGE_CONTENT``,
``RUN_FINISHED``, ...).
"""

import json
from typing import Any


def encode_event(event: dict[str, Any]) -> bytes:
    """Encode one AG-UI event object as a single SSE frame."""
    return f"data: {json.dumps(event, separators=(',', ':'))}\n\n".encode()
