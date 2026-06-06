from enum import StrEnum

from pydantic import Field, SecretStr
from pydantic_settings import BaseSettings, SettingsConfigDict


class AgentBackend(StrEnum):
    """Which agent implementation drives the AG-UI stream.

    ``STUB`` needs no API key and no AG2/OpenAI dependency tree: it emits a
    scripted, deterministic AG-UI event sequence (a safe tool call, a dangerous
    tool call, and a secret-looking token in text) so Agate's protections are
    demonstrable fully offline. ``AG2`` drives a real ``autogen.beta.Agent``
    over OpenAI (requires the ``ag2`` extra and an API key).
    """

    STUB = "stub"
    AG2 = "ag2"


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file_encoding="utf-8",
        env_nested_delimiter="__",
        env_prefix="AGENT__",
        extra="ignore",
    )

    app_name: str = "ag-ui-agent"

    # Default to the offline stub so `uv run ag-ui-agent` works with no secrets.
    backend: AgentBackend = AgentBackend.STUB

    openai_api_key: SecretStr = Field(default=SecretStr("sk-placeholder"))
    openai_model: str = "gpt-4o-mini"
    openai_proxy_url: str | None = None

    log_level: str = "INFO"
    log_json: bool = False
