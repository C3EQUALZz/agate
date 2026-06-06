"""The stub AG-UI stream — what Agate inspects in the protected demo.

These assertions pin the exact event shapes Agate's policy acts on: a safe
``search_documents`` call, a dangerous ``delete_file`` call, and assistant text
carrying an ``sk-...`` marker. If these change, the protected-demo expectations
must change too.
"""

import json

from fastapi.testclient import TestClient


def _events(client: TestClient, prompt: str = "find the api key") -> list[dict]:
    payload = {
        "threadId": "t1",
        "runId": "r1",
        "messages": [{"id": "m1", "role": "user", "content": prompt}],
        "state": {},
        "context": [],
        "tools": [],
        "forwardedProps": {},
    }
    with client.stream(
        "POST",
        "/api/run",
        json=payload,
        headers={"Accept": "text/event-stream"},
    ) as response:
        assert response.status_code == 200, response.read()
        body = b"".join(response.iter_bytes()).decode()

    events: list[dict] = []
    for frame in body.split("\n\n"):
        line = frame.strip()
        if line.startswith("data:"):
            events.append(json.loads(line[len("data:") :].strip()))
    return events


def test_stream_has_run_lifecycle(client: TestClient) -> None:
    types = [e["type"] for e in _events(client)]
    assert types[0] == "RUN_STARTED"
    assert types[-1] == "RUN_FINISHED"


def test_stream_emits_safe_and_dangerous_tool_calls(client: TestClient) -> None:
    tool_names = [
        e["toolCallName"] for e in _events(client) if e["type"] == "TOOL_CALL_START"
    ]
    assert "search_documents" in tool_names  # safe — Agate allows
    assert "delete_file" in tool_names  # dangerous — Agate denies


def test_stream_leaks_a_secret_marker_in_text(client: TestClient) -> None:
    text = "".join(
        e.get("delta", "") for e in _events(client) if e["type"] == "TEXT_MESSAGE_CONTENT"
    )
    assert "sk-" in text  # Agate redacts this before the client sees it
