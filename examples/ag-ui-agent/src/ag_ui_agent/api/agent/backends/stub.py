"""Offline stub backend: a scripted, deterministic AG-UI stream.

No autogen, no OpenAI, no API key. It drives the *real* use cases (so the
in-memory store genuinely mutates) and emits a fixed AG-UI event sequence
designed to exercise every Agate protection in one run:

1. ``search_documents`` — a SAFE tool call (Agate's allowlist permits it);
2. ``delete_file`` — a DANGEROUS tool call (Agate denies it);
3. assistant text containing a ``sk-...`` secret marker (Agate redacts it).

This makes the protected demo fully reproducible with zero external infra.
"""

from collections.abc import AsyncIterator
from typing import Any

import structlog

from ag_ui_agent.api.agent.run_input import RunAgentInputModel
from ag_ui_agent.api.agent.sse import encode_event
from ag_ui_agent.usecases import SearchDocumentsRequest, SearchDocumentsUseCase

logger = structlog.get_logger(__name__)

# A realistic-looking but fake credential. The leading "sk-" is the marker the
# demo's Agate policy redacts; nothing here is a real secret.
_LEAKED_SECRET = "sk-DEMO0000example0000fake0000token0000"  # pragma: allowlist secret
_SEARCH_CALL_ID = "call-search-1"
_DELETE_CALL_ID = "call-delete-1"
_MESSAGE_ID = "msg-stub-1"
_SEED_DOCUMENT_ID = "00000000-0000-0000-0000-000000000001"


class StubAgUiStreamer:
    """A scripted :class:`AgUiStreamer` for fully offline demos."""

    def __init__(self, search: SearchDocumentsUseCase) -> None:
        self._search = search

    async def dispatch(
        self,
        run_input: RunAgentInputModel,
        accept: str | None = None,
    ) -> AsyncIterator[bytes]:
        del accept  # The stub always emits text/event-stream framing.
        prompt = run_input.last_user_message()
        await logger.ainfo("stub.dispatch", run_id=run_input.run_id, prompt=prompt)

        yield _event("RUN_STARTED", threadId=run_input.thread_id, runId=run_input.run_id)

        # 1) SAFE tool call: search_documents — really runs the use case.
        async for frame in self._safe_search(prompt):
            yield frame

        # 2) DANGEROUS tool call: delete_file — Agate denies this one.
        async for frame in self._dangerous_delete():
            yield frame

        # 3) Assistant text that leaks a secret marker — Agate redacts it.
        async for frame in self._leaky_message():
            yield frame

        yield _event("RUN_FINISHED", threadId=run_input.thread_id, runId=run_input.run_id)

    async def _safe_search(self, prompt: str) -> AsyncIterator[bytes]:
        query = prompt or "key"
        yield _event("TOOL_CALL_START", toolCallId=_SEARCH_CALL_ID, toolCallName="search_documents")
        yield _event("TOOL_CALL_ARGS", toolCallId=_SEARCH_CALL_ID, delta=f'{{"query": "{query}"}}')
        yield _event("TOOL_CALL_END", toolCallId=_SEARCH_CALL_ID)

        response = await self._search.execute(SearchDocumentsRequest(query=query, limit=5))
        names = ", ".join(d.name for d in response.documents) or "(no matches)"
        yield _event("TOOL_CALL_RESULT", toolCallId=_SEARCH_CALL_ID, content=f"found: {names}")

    async def _dangerous_delete(self) -> AsyncIterator[bytes]:
        # Deliberately does NOT run the delete use case. In the protected demo
        # Agate denies this call upstream of any side effect; the stub only
        # *announces* the dangerous call so the proxy has something to block.
        yield _event("TOOL_CALL_START", toolCallId=_DELETE_CALL_ID, toolCallName="delete_file")
        yield _event("TOOL_CALL_ARGS", toolCallId=_DELETE_CALL_ID, delta=f'{{"document_id": "{_SEED_DOCUMENT_ID}"}}')
        yield _event("TOOL_CALL_END", toolCallId=_DELETE_CALL_ID)
        yield _event("TOOL_CALL_RESULT", toolCallId=_DELETE_CALL_ID, content="deleted")

    async def _leaky_message(self) -> AsyncIterator[bytes]:
        text = (
            "Done. I searched the workspace and (attempted to) clean up. "
            f"For reference, the staging key is {_LEAKED_SECRET} — keep it safe."
        )
        yield _event("TEXT_MESSAGE_START", messageId=_MESSAGE_ID, role="assistant")
        yield _event("TEXT_MESSAGE_CONTENT", messageId=_MESSAGE_ID, delta=text)
        yield _event("TEXT_MESSAGE_END", messageId=_MESSAGE_ID)


def _event(event_type: str, **fields: Any) -> bytes:
    return encode_event({"type": event_type, **fields})
