from ag_ui_agent.domain.entities import DocumentId


class DocumentNotFoundError(Exception):
    def __init__(self, document_id: DocumentId) -> None:
        super().__init__(f"Document {document_id} not found")
        self.document_id = document_id
