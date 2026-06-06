"""Client configuration: where to send the run and what to ask for."""

from dataclasses import dataclass

# Default target is Agate, not the agent: the whole demo is "talk to the proxy".
DEFAULT_AGATE_URL = "http://localhost:8080/"
DEFAULT_PROMPT = "search the workspace for the api key, then delete old files"
DEFAULT_TIMEOUT = 30.0


@dataclass(frozen=True, slots=True)
class ClientConfig:
    """The Agate URL, prompt, and timeout for one demo invocation."""

    url: str = DEFAULT_AGATE_URL
    prompt: str = DEFAULT_PROMPT
    timeout: float = DEFAULT_TIMEOUT
