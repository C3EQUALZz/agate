from protected_demo_client.domain.observation import Observation
from protected_demo_client.transport.sse import parse_sse_lines


def test_parse_sse_lines_decodes_frames() -> None:
    raw = [
        'data: {"type": "RUN_STARTED"}',
        "",
        "data: {",
        'data: "type": "TOOL_CALL_START", "toolCallName": "search_documents"}',
        "",
    ]
    events = list(parse_sse_lines(iter(raw)))
    assert events[0] == {"type": "RUN_STARTED"}
    assert events[1]["toolCallName"] == "search_documents"


def test_observation_infers_agate_protections() -> None:
    # The stream Agate would produce under the demo policy: the dangerous call
    # is absent, the secret is redacted, and the run ends in RUN_ERROR.
    obs = Observation()
    for event in [
        {"type": "RUN_STARTED"},
        {"type": "TOOL_CALL_START", "toolCallName": "search_documents", "toolCallId": "c1"},
        {"type": "TOOL_CALL_RESULT", "toolCallId": "c1", "content": "found"},
        {"type": "TEXT_MESSAGE_CONTENT", "messageId": "m1", "delta": "key is [REDACTED]"},
        {"type": "RUN_ERROR", "message": "tool 'delete_file' denied by policy"},
    ]:
        obs.observe(event)

    assert obs.allowed_tool_passed
    assert obs.dangerous_tool_blocked
    assert obs.saw_redaction
    assert obs.saw_run_error


def test_observation_flags_unblocked_dangerous_call() -> None:
    obs = Observation()
    obs.observe({"type": "TOOL_CALL_START", "toolCallName": "delete_file", "toolCallId": "c2"})
    assert not obs.dangerous_tool_blocked
