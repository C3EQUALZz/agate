"""Send an AG-UI run through Agate and print the inspected event stream.

Usage:

    uv run protected-demo-client                       # -> http://localhost:8080/
    uv run protected-demo-client --url http://host:8080/
    uv run protected-demo-client --prompt "find the readme"

What to look for in the output (with the demo's agate.toml policy):

  * the assistant text has the ``sk-...`` secret replaced with ``[REDACTED]``;
  * the ``delete_file`` tool call never reaches you — Agate denies it and the
    run ends in a ``RUN_ERROR`` (the dangerous action was blocked);
  * the allowed ``search`` tool call passes through unchanged;
  * every one of those (event, verdict) decisions was recorded to Agate's
    transparency log (see ../audit-verify).
"""

from __future__ import annotations

import argparse
import json
import sys
from typing import Any

import httpx

from protected_demo_client.sse import parse_sse_lines

# Default: Agate, not the agent. The whole demo is "talk to the proxy".
DEFAULT_URL = "http://localhost:8080/"

# ANSI colors (no dependency); disabled when stdout is not a TTY.
_TTY = sys.stdout.isatty()
DIM = "\033[2m" if _TTY else ""
BOLD = "\033[1m" if _TTY else ""
GREEN = "\033[32m" if _TTY else ""
YELLOW = "\033[33m" if _TTY else ""
RED = "\033[31m" if _TTY else ""
RESET = "\033[0m" if _TTY else ""


def build_run_input(prompt: str) -> dict[str, Any]:
    """A minimal AG-UI ``RunAgentInput`` body."""
    return {
        "threadId": "demo-thread",
        "runId": "demo-run",
        "messages": [{"role": "user", "content": prompt}],
    }


def describe(event: dict[str, Any]) -> str:
    """Render one AG-UI event as a readable, annotated line."""
    kind = event.get("type", "?")

    if kind == "TEXT_MESSAGE_CONTENT":
        delta = event.get("delta", "")
        note = ""
        if "[REDACTED]" in delta or "REDACTED" in delta:
            note = f"  {GREEN}<- secret redacted by Agate{RESET}"
        return f"{BOLD}{kind}{RESET} delta={delta!r}{note}"

    if kind == "TOOL_CALL_START":
        name = event.get("toolCallName", "?")
        return f"{BOLD}{kind}{RESET} name={YELLOW}{name}{RESET} id={event.get('toolCallId')}"

    if kind == "TOOL_CALL_ARGS":
        return f"{kind} delta={event.get('delta')!r}"

    if kind == "RUN_ERROR":
        msg = event.get("message", "")
        return f"{RED}{BOLD}{kind}{RESET} message={msg!r}  {RED}<- run blocked by Agate{RESET}"

    if kind in {"RUN_STARTED", "RUN_FINISHED"}:
        return f"{DIM}{kind}{RESET}"

    return f"{kind} {DIM}{json.dumps({k: v for k, v in event.items() if k != 'type'})}{RESET}"


def run(url: str, prompt: str, timeout: float) -> int:
    body = build_run_input(prompt)
    print(f"{DIM}POST {url}{RESET}")
    print(f"{DIM}body {json.dumps(body)}{RESET}\n")

    saw_delete = False
    saw_redaction = False
    saw_error = False

    try:
        with httpx.Client(timeout=timeout) as client:
            with client.stream("POST", url, json=body) as response:
                if response.status_code != 200:
                    response.read()
                    print(f"{RED}HTTP {response.status_code}{RESET}: {response.text}")
                    return 1
                for event in parse_sse_lines(response.iter_lines()):
                    print("  " + describe(event))
                    if event.get("type") == "TOOL_CALL_START":
                        saw_delete = saw_delete or event.get("toolCallName") == "delete_file"
                    if event.get("type") == "TEXT_MESSAGE_CONTENT":
                        saw_redaction = saw_redaction or "REDACTED" in event.get("delta", "")
                    if event.get("type") == "RUN_ERROR":
                        saw_error = True
    except httpx.HTTPError as error:
        print(f"{RED}request failed{RESET}: {error}")
        print(f"{DIM}is the demo up?  docker compose up --build{RESET}")
        return 1

    _summary(saw_redaction, saw_delete, saw_error)
    return 0


def _summary(saw_redaction: bool, saw_delete: bool, saw_error: bool) -> None:
    print(f"\n{BOLD}What Agate did{RESET}")
    _line(saw_redaction, "secret marker redacted to [REDACTED] in assistant text")
    _line(not saw_delete, "dangerous 'delete_file' tool call NOT forwarded (denied)")
    _line(saw_error, "run terminated with RUN_ERROR after the denied tool call")
    print(
        f"{DIM}Every (event, verdict) decision above was appended to Agate's "
        f"transparency log; see ../audit-verify to inspect it.{RESET}"
    )


def _line(ok: bool, text: str) -> None:
    mark = f"{GREEN}OK{RESET}" if ok else f"{YELLOW}--{RESET}"
    print(f"  [{mark}] {text}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Send an AG-UI run through Agate.")
    parser.add_argument("--url", default=DEFAULT_URL, help=f"Agate URL (default {DEFAULT_URL})")
    parser.add_argument(
        "--prompt",
        default="search the docs and clean up old files",
        help="user prompt to send",
    )
    parser.add_argument("--timeout", type=float, default=30.0, help="HTTP timeout (s)")
    args = parser.parse_args()
    raise SystemExit(run(args.url, args.prompt, args.timeout))


if __name__ == "__main__":
    main()
