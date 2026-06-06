"""HTTP transport: POST an AG-UI run to Agate and stream back its events."""

from __future__ import annotations

from collections.abc import Iterator
from http import HTTPStatus
from typing import Any

import httpx

from protected_demo_client.transport.sse import parse_sse_lines


def build_run_input(prompt: str) -> dict[str, Any]:
    """A minimal AG-UI ``RunAgentInput`` body."""
    return {
        "threadId": "demo-thread",
        "runId": "demo-run",
        "messages": [{"id": "m1", "role": "user", "content": prompt}],
        "state": {},
        "context": [],
        "tools": [],
        "forwardedProps": {},
    }


class AgateClientError(RuntimeError):
    """Raised when Agate is unreachable or returns a non-200 status."""


def stream_run(url: str, prompt: str, timeout: float) -> Iterator[dict[str, Any]]:
    """Yield the AG-UI events Agate emits for one run.

    Raises :class:`AgateClientError` on transport failure or a non-200 response.
    """
    body = build_run_input(prompt)
    try:
        with (
            httpx.Client(timeout=timeout) as client,
            client.stream("POST", url, json=body) as response,
        ):
            if response.status_code != HTTPStatus.OK:
                response.read()
                raise AgateClientError(f"HTTP {response.status_code}: {response.text}")
            yield from parse_sse_lines(response.iter_lines())
    except httpx.HTTPError as error:
        raise AgateClientError(str(error)) from error
