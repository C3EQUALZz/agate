r"""A tiny Server-Sent-Events parser for the AG-UI stream.

SSE frames are separated by a blank line; ``data:`` lines within a frame are
concatenated. For AG-UI the concatenated data is one JSON event object. This is
the exact framing Agate emits (``data: {json}\n\n``).
"""

from __future__ import annotations

import json
from collections.abc import Iterator
from typing import Any


def parse_sse_lines(lines: Iterator[str]) -> Iterator[dict[str, Any]]:
    """Yield decoded JSON events from an iterator of raw SSE lines.

    Lines arrive without trailing newlines (as httpx's ``iter_lines`` yields
    them). A blank line terminates a frame; the accumulated ``data`` is decoded.
    """
    data_parts: list[str] = []
    for line in lines:
        if line == "":
            if data_parts:
                yield _decode("".join(data_parts))
                data_parts = []
            continue
        chunk = _data_field(line)
        if chunk is not None:
            data_parts.append(chunk)
    if data_parts:  # Stream ended without a trailing blank line.
        yield _decode("".join(data_parts))


def _data_field(line: str) -> str | None:
    """Return an SSE line's ``data:`` payload, or ``None`` for other lines."""
    if line.startswith(":"):  # SSE comment / keep-alive.
        return None
    field, _, payload = line.partition(":")
    if field != "data":
        return None
    return payload.removeprefix(" ")


def _decode(payload: str) -> dict[str, Any]:
    try:
        decoded = json.loads(payload)
    except json.JSONDecodeError:
        return {"type": "_UNPARSABLE", "raw": payload}
    return decoded if isinstance(decoded, dict) else {"type": "_NON_OBJECT", "raw": payload}
