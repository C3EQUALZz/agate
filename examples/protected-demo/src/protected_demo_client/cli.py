"""Send an AG-UI run through Agate and print the inspected event stream.

Usage:

    uv run protected-demo-client                       # -> http://localhost:8080/
    uv run protected-demo-client --url http://host:8080/
    uv run protected-demo-client --prompt "find the readme"

What to look for in the output (with the demo's agate.toml policy):

  * the assistant text has the ``sk-...`` secret replaced with ``[REDACTED]``;
  * the ``delete_file`` tool call never reaches you — Agate denies it and the
    run ends in a ``RUN_ERROR`` (the dangerous action was blocked);
  * the allowed ``search_documents`` tool call passes through unchanged;
  * every one of those (event, verdict) decisions was recorded to Agate's
    transparency log (see ../audit-verify).
"""

from __future__ import annotations

import argparse

from protected_demo_client import render
from protected_demo_client.config import DEFAULT_AGATE_URL, DEFAULT_PROMPT, ClientConfig
from protected_demo_client.domain.observation import Observation
from protected_demo_client.transport.agate import (
    AgateClientError,
    build_run_input,
    stream_run,
)


def run(config: ClientConfig) -> int:
    render.header(config.url, build_run_input(config.prompt))
    observation = Observation()
    try:
        for event in stream_run(config.url, config.prompt, config.timeout):
            render.event_line(event)
            observation.observe(event)
    except AgateClientError as error:
        render.error(str(error))
        return 1
    render.summary(observation)
    return 0


def parse_args(argv: list[str] | None = None) -> ClientConfig:
    parser = argparse.ArgumentParser(description="Send an AG-UI run through Agate.")
    parser.add_argument("--url", default=DEFAULT_AGATE_URL, help=f"Agate URL (default {DEFAULT_AGATE_URL})")
    parser.add_argument("--prompt", default=DEFAULT_PROMPT, help="user prompt to send")
    parser.add_argument("--timeout", type=float, default=30.0, help="HTTP timeout (s)")
    args = parser.parse_args(argv)
    return ClientConfig(url=args.url, prompt=args.prompt, timeout=args.timeout)


def main() -> None:
    raise SystemExit(run(parse_args()))


if __name__ == "__main__":
    main()
