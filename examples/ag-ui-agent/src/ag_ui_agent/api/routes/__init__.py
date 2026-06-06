from ag_ui_agent.api.routes.chat import router as chat_router
from ag_ui_agent.api.routes.documents import router as documents_router
from ag_ui_agent.api.routes.health import router as health_router

routers = [health_router, documents_router, chat_router]

__all__ = ["routers"]
