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
        return "/api/health" not in record.getMessage()


def configure_logging(level: str = "INFO", json_output: bool = False) -> None:
    log_level = getattr(logging, level.upper(), logging.INFO)

    shared_processors: list[Processor] = [
        structlog.contextvars.merge_contextvars,
        add_logger_name,
        add_log_level,
        TimeStamper(fmt="iso", utc=True),
        structlog.processors.StackInfoRenderer(),
        structlog.dev.set_exc_info,
    ]

    renderer: structlog.typing.Processor
    if json_output:
        renderer = JSONRenderer()
        shared_processors.append(structlog.processors.format_exc_info)
    else:
        renderer = structlog.dev.ConsoleRenderer()

    structlog.configure(
        processors=shared_processors + [ProcessorFormatter.wrap_for_formatter],
        context_class=dict,
        logger_factory=LoggerFactory(),
        wrapper_class=structlog.make_filtering_bound_logger(log_level),
        cache_logger_on_first_use=True,
    )

    formatter = ProcessorFormatter(
        processor=renderer,
        foreign_pre_chain=shared_processors + [ExtraAdder()],
    )

    logging.basicConfig(level=log_level, handlers=[logging.StreamHandler(sys.stdout)])
    for handler in logging.getLogger().handlers:
        handler.setFormatter(formatter)

    for logger_name in ["uvicorn", "uvicorn.access", "uvicorn.error", "fastapi"]:
        logging.getLogger(logger_name).handlers.clear()
        logging.getLogger(logger_name).propagate = True

    logging.getLogger("uvicorn.access").addFilter(HealthCheckAccessFilter())
