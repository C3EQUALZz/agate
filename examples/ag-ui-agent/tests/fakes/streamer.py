"""A fake :class:`AgUiStreamer` for integration tests.

The one real backend drives a live OpenAI model, which tests must not call. This
fake reproduces the AG-UI event sequence the demo relies on -- a safe
``search_documents`` call, a dangerous ``delete_file`` call, and assistant text
carrying an ``sk-...`` marker -- while genuinely running the real
``SearchDocumentsUseCase`` so the wiring (Dishka resolution, the route, SSE
framing) is exercised end to end without autogen or a network call.
"""

import json
from collections.abc import AsyncIterator
from typing import Any, Final

from ag_ui_agent.api.agent.prompts import FAKE_STAGING_KEY
from ag_ui_agent.api.agent.run_input import RunAgentInputModel
from ag_ui_agent.usecases import SearchDocumentsRequest, SearchDocumentsUseCase

_SEARCH_CALL_ID: Final = "call-search-1"
_DELETE_CALL_ID: Final = "call-delete-1"
_MESSAGE_ID: Final = "msg-fake-1"
_SEED_DOCUMENT_ID: Final = "00000000-0000-0000-0000-000000000001"
_SEARCH_LIMIT: Final = 5


def _frame(event_type: str, **fields: Any) -> bytes:
    event = {"type": event_type, **fields}
    return f"data: {json.dumps(event, separators=(',', ':'))}\n\n".encode()


class FakeAgUiStreamer:
    """Scripted AG-UI streamer that still drives the real search use case."""

    def __init__(self, search: SearchDocumentsUseCase) -> None:
        self._search = search

    async def dispatch(
        self,
        run_input: RunAgentInputModel,
        accept: str | None = None,
    ) -> AsyncIterator[bytes]:
        del accept  # The fake always emits text/event-stream framing.
        yield _frame("RUN_STARTED", threadId=run_input.thread_id, runId=run_input.run_id)
        async for frame in self._safe_search(run_input.last_user_message()):
            yield frame
        async for frame in self._dangerous_delete():
            yield frame
        async for frame in self._leaky_message():
            yield frame
        yield _frame("RUN_FINISHED", threadId=run_input.thread_id, runId=run_input.run_id)

    async def _safe_search(self, prompt: str) -> AsyncIterator[bytes]:
        query = prompt or "key"
        yield _frame("TOOL_CALL_START", toolCallId=_SEARCH_CALL_ID, toolCallName="search_documents")
        yield _frame("TOOL_CALL_ARGS", toolCallId=_SEARCH_CALL_ID, delta=f'{{"query": "{query}"}}')
        yield _frame("TOOL_CALL_END", toolCallId=_SEARCH_CALL_ID)
        response = await self._search.execute(
            SearchDocumentsRequest(query=query, limit=_SEARCH_LIMIT),
        )
        names = ", ".join(doc.name for doc in response.documents) or "(no matches)"
        yield _frame("TOOL_CALL_RESULT", toolCallId=_SEARCH_CALL_ID, content=f"found: {names}")

    async def _dangerous_delete(self) -> AsyncIterator[bytes]:
        yield _frame("TOOL_CALL_START", toolCallId=_DELETE_CALL_ID, toolCallName="delete_file")
        yield _frame(
            "TOOL_CALL_ARGS",
            toolCallId=_DELETE_CALL_ID,
            delta=f'{{"document_id": "{_SEED_DOCUMENT_ID}"}}',
        )
        yield _frame("TOOL_CALL_END", toolCallId=_DELETE_CALL_ID)

    async def _leaky_message(self) -> AsyncIterator[bytes]:
        text = f"Done. For reference, the staging key is {FAKE_STAGING_KEY} -- keep it safe."
        yield _frame("TEXT_MESSAGE_START", messageId=_MESSAGE_ID, role="assistant")
        yield _frame("TEXT_MESSAGE_CONTENT", messageId=_MESSAGE_ID, delta=text)
        yield _frame("TEXT_MESSAGE_END", messageId=_MESSAGE_ID)
