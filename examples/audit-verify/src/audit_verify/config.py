from dataclasses import dataclass

# The protected-demo's Postgres. It is not published to the host by default;
# add `ports: ["5432:5432"]` to that compose file, or run inside the network.
DEFAULT_DATABASE_URL = "postgres://agate:agate@localhost:5432/agate"


@dataclass(frozen=True, slots=True)
class Config:
    database_url: str = DEFAULT_DATABASE_URL
    connect_timeout: int = 5
    sample_leaves: int = 10
