"""ASGI middleware that opens an AG2 request scope per HTTP request.

The container is scoped with ``dishka_ag2.AG2Scope`` rather than dishka's default
``Scope``. dishka's stock FastAPI integration opens ``Scope.REQUEST`` and so
cannot drive ``AG2Scope``; this mirror of the reference's
``AG2ContainerMiddleware`` opens ``AG2Scope.REQUEST`` and stashes the request
container on ``request.state`` so ``dishka.integrations.fastapi``'s ``@inject``
keeps working for REST routes.
"""

from dishka import AsyncContainer
from dishka_ag2 import AG2Scope
from starlette.requests import Request
from starlette.types import ASGIApp, Receive, Scope, Send


class AG2ContainerMiddleware:
    """Open an ``AG2Scope.REQUEST`` child container for each HTTP request."""

    def __init__(self, app: ASGIApp, container: AsyncContainer) -> None:
        self.app = app
        self.container = container

    async def __call__(self, scope: Scope, receive: Receive, send: Send) -> None:
        """Wrap each HTTP request in an ``AG2Scope.REQUEST`` container."""
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return

        request = Request(scope, receive, send)
        async with self.container(
            context={Request: request},
            scope=AG2Scope.REQUEST,
        ) as request_container:
            request.state.dishka_container = request_container
            await self.app(scope, receive, send)
