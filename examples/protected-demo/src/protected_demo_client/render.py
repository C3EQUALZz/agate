"""Terminal rendering of the AG-UI event stream and the protection summary.

Pure presentation: no I/O beyond ``print``. ANSI colors are disabled when stdout
is not a TTY, so piped output stays clean.
"""

from __future__ import annotations

import json
import sys
from typing import Any

from protected_demo_client.domain.observation import Observation

_TTY = sys.stdout.isatty()
DIM = "\033[2m" if _TTY else ""
BOLD = "\033[1m" if _TTY else ""
GREEN = "\033[32m" if _TTY else ""
YELLOW = "\033[33m" if _TTY else ""
RED = "\033[31m" if _TTY else ""
RESET = "\033[0m" if _TTY else ""


def describe(event: dict[str, Any]) -> str:
    """Render one AG-UI event as a readable, annotated line."""
    kind = str(event.get("type", "?"))

    if kind == "TEXT_MESSAGE_CONTENT":
        delta = str(event.get("delta", ""))
        note = ""
        if "REDACTED" in delta:
            note = f"  {GREEN}<- secret redacted by Agate{RESET}"
        return f"{BOLD}{kind}{RESET} delta={delta!r}{note}"

    if kind == "TOOL_CALL_START":
        name = str(event.get("toolCallName", "?"))
        return f"{BOLD}{kind}{RESET} name={YELLOW}{name}{RESET} id={event.get('toolCallId')}"

    if kind == "TOOL_CALL_ARGS":
        return f"{kind} delta={event.get('delta')!r}"

    if kind == "RUN_ERROR":
        msg = event.get("message", "")
        return f"{RED}{BOLD}{kind}{RESET} message={msg!r}  {RED}<- run blocked by Agate{RESET}"

    if kind in {"RUN_STARTED", "RUN_FINISHED"}:
        return f"{DIM}{kind}{RESET}"

    rest = {field: field_value for field, field_value in event.items() if field != "type"}
    return f"{kind} {DIM}{json.dumps(rest)}{RESET}"


def header(url: str, body: dict[str, Any]) -> None:
    """Print the request line and body the demo is about to send."""
    print(f"{DIM}POST {url}{RESET}")
    print(f"{DIM}body {json.dumps(body)}{RESET}\n")


def event_line(event: dict[str, Any]) -> None:
    """Print one inspected AG-UI event as an annotated line."""
    print(f"  {describe(event)}")


def error(message: str) -> None:
    """Print a transport-failure message with a hint to start the demo."""
    print(f"{RED}request failed{RESET}: {message}")
    print(f"{DIM}is the demo up?  docker compose up --build{RESET}")


def summary(observation: Observation) -> None:
    """Print the "what Agate did" summary inferred from the stream."""
    print(f"\n{BOLD}What Agate did{RESET}")
    _line("secret marker redacted to [REDACTED] in assistant text", ok=observation.saw_redaction)
    _line(
        "dangerous 'delete_file' tool call NOT forwarded (denied)",
        ok=observation.dangerous_tool_blocked,
    )
    _line("run terminated with RUN_ERROR after the denied tool call", ok=observation.saw_run_error)
    _line("allowed 'search_documents' tool call passed through", ok=observation.allowed_tool_passed)
    print(
        f"{DIM}Every (event, verdict) decision above was appended to Agate's "
        f"transparency log; see ../audit-verify to inspect it.{RESET}"
    )


def _line(text: str, *, ok: bool) -> None:
    """Print a checklist line: a green ``OK`` if ``ok`` else a dim ``--``."""
    mark = f"{GREEN}OK{RESET}" if ok else f"{YELLOW}--{RESET}"
    print(f"  [{mark}] {text}")
