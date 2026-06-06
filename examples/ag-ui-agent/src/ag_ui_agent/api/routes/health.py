"""The liveness probe route."""

from fastapi import APIRouter

router = APIRouter(tags=["system"])


@router.get("/health")
async def health() -> dict[str, str]:
    """Return a static liveness payload."""
    return {"status": "healthy"}
