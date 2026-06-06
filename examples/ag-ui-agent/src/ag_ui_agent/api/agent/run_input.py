"""A minimal, framework-neutral model of the AG-UI ``RunAgentInput`` body.

The chat route parses the incoming JSON into this Pydantic model so it never
imports ``autogen``; the AG2 backend rebuilds the native
``autogen.beta.ag_ui.RunAgentInput`` from this model's wire form. This keeps the
HTTP layer framework-light and honours the import-linter contract that
``autogen`` lives only under ``api.agent``.

See the AG-UI protocol: https://docs.ag-ui.com/concepts/events
"""

from typing import Any

from pydantic import BaseModel, ConfigDict, Field


class RunAgentMessage(BaseModel):
    """One AG-UI conversation message."""

    id: str | None = None
    role: str
    content: str = ""


class RunAgentInputModel(BaseModel):
    """The AG-UI ``RunAgentInput`` request body, parsed framework-neutrally."""

    thread_id: str = Field(default="thread", alias="threadId")
    run_id: str = Field(default="run", alias="runId")
    messages: list[RunAgentMessage] = Field(default_factory=list)
    state: dict[str, Any] = Field(default_factory=dict)
    context: list[Any] = Field(default_factory=list)
    tools: list[Any] = Field(default_factory=list)
    forwarded_props: dict[str, Any] = Field(default_factory=dict, alias="forwardedProps")

    model_config = ConfigDict(populate_by_name=True, extra="allow")

    def last_user_message(self) -> str:
        """Return the most recent user message's content (or empty)."""
        for message in reversed(self.messages):
            if message.role == "user":
                return message.content
        return ""
