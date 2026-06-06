"""DI wiring for the agent's toolkit.

The toolkit is assembled by Dishka rather than hand-constructed at the agent
factory: the ``ToolkitProvider`` is the single place that decides which tools
the agent gets, and the agent factory resolves a ready ``Toolkit`` from the
container. The ``@tool @inject`` functions themselves resolve their use-case
collaborators (``FromDishka[...]``) from an ``AG2Scope.REQUEST`` child container
that ``DishkaAsyncMiddleware`` opens per tool call.
"""

from autogen.beta import Toolkit
from dishka import Provider, provide
from dishka_ag2 import AG2Scope

from ag_ui_agent.api.agent.tools import (
    delete_file,
    echo_status,
    list_documents,
    search_documents,
)


class ToolkitProvider(Provider):
    """Provide the agent's :class:`Toolkit` as an app-scoped singleton."""

    @provide(scope=AG2Scope.APP)
    def provide_toolkit(self) -> Toolkit:
        """Build the toolkit from the agent's ``@tool @inject`` functions."""
        return Toolkit(
            echo_status,
            search_documents,
            list_documents,
            delete_file,
        )
