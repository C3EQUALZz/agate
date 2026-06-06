"""Domain/use-case level errors."""

from ag_ui_agent.domain.entities import DocumentId


class DocumentNotFoundError(Exception):
    """Raised when an operation targets a document id that does not exist."""

    def __init__(self, document_id: DocumentId) -> None:
        super().__init__(f"Document {document_id} not found")
        self.document_id = document_id
