from dataclasses import dataclass

# Default target is Agate, not the agent: the whole demo is "talk to the proxy".
DEFAULT_AGATE_URL = "http://localhost:8080/"
DEFAULT_PROMPT = "search the workspace for the api key, then delete old files"


@dataclass(frozen=True, slots=True)
class ClientConfig:
    url: str = DEFAULT_AGATE_URL
    prompt: str = DEFAULT_PROMPT
    timeout: float = 30.0
