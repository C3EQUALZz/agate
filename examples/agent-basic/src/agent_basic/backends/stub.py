"""A scripted AG-UI agent — no LLM, no API key, fully deterministic.

This backend exists so the whole ``protected-demo`` runs offline and shows
Agate's protections reproducibly. Each run emits, in order:

1. ``RUN_STARTED``
2. assistant text that **embeds a secret marker** (so Agate can redact it),
3. a ``search`` tool call (allowed by the demo policy),
4. a ``delete_file`` tool call (**denied** by the demo policy — this is the
   dangerous action Agate blocks),
5. ``RUN_FINISHED``.

Put behind Agate with ``[policy.tools] mode = "allowlist", names = ["search"]``
and ``[policy] redact = ["sk-"]``, the client sees: the secret masked to
``[REDACTED]`` and the ``delete_file`` call dropped (surfaced as ``RUN_ERROR``),
while ``search`` passes through.
"""

from __future__ import annotations

import uuid
from collections.abc import AsyncIterator
from typing import Any

from agent_basic import ag_ui
from agent_basic.config import AgentConfig
from agent_basic.run_input import RunAgentInput


class StubBackend:
    """Emits a fixed, illustrative AG-UI event script for any input."""

    def __init__(self, config: AgentConfig) -> None:
        self._config = config

    async def run(self, request: RunAgentInput) -> AsyncIterator[dict[str, Any]]:
        prompt = request.last_user_message or "(no prompt)"

        yield ag_ui.run_started(request.thread_id, request.run_id)

        # 1) Assistant text that leaks a secret — Agate's redaction target.
        #    The secret is emitted in ONE content frame (not chunked) so the
        #    "sk-" marker stays intact within a single TEXT_MESSAGE_CONTENT for
        #    Agate to match and redact; the surrounding prose is streamed.
        message_id = f"msg-{uuid.uuid4().hex[:8]}"
        yield ag_ui.text_message_start(message_id)
        yield ag_ui.text_message_content(message_id, f"You asked: {prompt!r}. ")
        yield ag_ui.text_message_content(
            message_id,
            "Here is a (fake) credential you should never see in plaintext: "
            f"{self._config.demo_secret}",
        )
        yield ag_ui.text_message_content(message_id, ". I'll search and tidy up.")
        yield ag_ui.text_message_end(message_id)

        # 2) An allowed tool call: "search".
        for event in ag_ui.call_tool(
            f"call-{uuid.uuid4().hex[:8]}", "search", {"query": prompt}
        ):
            yield event

        # 3) A DANGEROUS tool call: "delete_file" — not on the allowlist, so
        #    Agate denies it. Without Agate the agent would happily emit it.
        for event in ag_ui.call_tool(
            f"call-{uuid.uuid4().hex[:8]}", "delete_file", {"path": "/etc/passwd"}
        ):
            yield event

        yield ag_ui.run_finished(request.thread_id, request.run_id)
