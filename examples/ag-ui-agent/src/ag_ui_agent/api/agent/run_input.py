"""A minimal, framework-neutral model of the AG-UI ``RunAgentInput`` body.

The real-AG2 backend uses ``autogen.beta.ag_ui.RunAgentInput`` directly. The
stub backend has no autogen dependency, so the chat route parses the incoming
JSON into this Pydantic model instead, and the backend port accepts the raw
mapping. This keeps the wire contract identical across both backends while
letting the route stay framework-light.

See the AG-UI protocol: https://docs.ag-ui.com/concepts/events
"""

from typing import Any

from pydantic import BaseModel, ConfigDict, Field


class RunAgentMessage(BaseModel):
    id: str | None = None
    role: str
    content: str = ""


class RunAgentInputModel(BaseModel):
    thread_id: str = Field(default="thread", alias="threadId")
    run_id: str = Field(default="run", alias="runId")
    messages: list[RunAgentMessage] = Field(default_factory=list)
    state: dict[str, Any] = Field(default_factory=dict)
    context: list[Any] = Field(default_factory=list)
    tools: list[Any] = Field(default_factory=list)
    forwarded_props: dict[str, Any] = Field(default_factory=dict, alias="forwardedProps")

    model_config = ConfigDict(populate_by_name=True, extra="allow")

    def last_user_message(self) -> str:
        for message in reversed(self.messages):
            if message.role == "user":
                return message.content
        return ""
