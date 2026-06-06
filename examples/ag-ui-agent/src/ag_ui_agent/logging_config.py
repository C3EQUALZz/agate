"""structlog + stdlib logging configuration."""

import logging
import sys

import structlog
from structlog.processors import JSONRenderer, TimeStamper
from structlog.stdlib import (
    ExtraAdder,
    LoggerFactory,
    ProcessorFormatter,
    add_log_level,
    add_logger_name,
)
from structlog.typing import Processor


class HealthCheckAccessFilter(logging.Filter):
    """Drop access-log lines for the health probe to keep logs readable."""

    def filter(self, record: logging.LogRecord) -> bool:
        """Return ``False`` for health-probe access lines (drop them)."""
        return "/api/health" not in record.getMessage()


# stdlib loggers whose own handlers we clear so records flow through structlog.
_NOISY_LOGGERS = ("uvicorn", "uvicorn.access", "uvicorn.error", "fastapi")


def _shared_processors() -> list[Processor]:
    """The structlog processors shared by both console and JSON output."""
    return [
        structlog.contextvars.merge_contextvars,
        add_logger_name,
        add_log_level,
        TimeStamper(fmt="iso", utc=True),
        structlog.processors.StackInfoRenderer(),
        structlog.dev.set_exc_info,
    ]


def _silence_stdlib_loggers() -> None:
    """Route uvicorn/fastapi records through the root structlog handler."""
    for logger_name in _NOISY_LOGGERS:
        logging.getLogger(logger_name).handlers.clear()
        logging.getLogger(logger_name).propagate = True


def configure_logging(level: str = "INFO", json_output: bool = False) -> None:
    """Configure structlog + stdlib logging (console or JSON)."""
    log_level = getattr(logging, level.upper(), logging.INFO)
    shared = _shared_processors()

    renderer: Processor
    if json_output:
        renderer = JSONRenderer()
        shared.append(structlog.processors.format_exc_info)
    else:
        renderer = structlog.dev.ConsoleRenderer()

    structlog.configure(
        processors=[*shared, ProcessorFormatter.wrap_for_formatter],
        context_class=dict,
        logger_factory=LoggerFactory(),
        wrapper_class=structlog.make_filtering_bound_logger(log_level),
        cache_logger_on_first_use=True,
    )

    formatter = ProcessorFormatter(
        processor=renderer,
        foreign_pre_chain=[*shared, ExtraAdder()],
    )
    logging.basicConfig(level=log_level, handlers=[logging.StreamHandler(sys.stdout)])
    for log_handler in logging.getLogger().handlers:
        log_handler.setFormatter(formatter)
    _silence_stdlib_loggers()

    logging.getLogger("uvicorn.access").addFilter(HealthCheckAccessFilter())
