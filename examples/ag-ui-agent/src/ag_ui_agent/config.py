"""Application settings (environment-driven)."""

from pydantic import SecretStr
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Application settings, read from the environment (``AGENT__`` prefix).

    The agent drives a real ``autogen.beta`` (AG2) agent over OpenAI, so an
    OpenAI API key is mandatory: there is no offline mode. Supply it via
    ``AGENT__OPENAI_API_KEY`` (or an ``.env`` file).
    """

    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        env_nested_delimiter="__",
        env_prefix="AGENT__",
        extra="ignore",
    )

    app_name: str = "ag-ui-agent"

    openai_api_key: SecretStr
    openai_model: str = "gpt-4o-mini"
    openai_proxy_url: str | None = None
    # Override the API base URL to target any OpenAI-compatible provider (e.g.
    # Mistral at https://api.mistral.ai/v1). None uses OpenAI's default.
    openai_base_url: str | None = None

    log_level: str = "INFO"
    log_json: bool = False
