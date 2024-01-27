use super::lsp_client::*;
use anyhow::anyhow;
use lsp_types::*;
use serde_json::from_value;

trait LspEditor
where
  Self: LspClient,
{
  /// Prepares a `WorkspaceEdit` object that encapsulates all intended modifications,
  /// such as computing diffs and determining necessary text edits, organized per document.
  /// @param parameters to define the changes.
  /// @return `WorkspaceEdit` with all prepared edits.
  async fn prepare_edits(&self /* parameters defining the changes */) -> anyhow::Result<WorkspaceEdit>;

  /// Validates the prepared edits to ensure they can be correctly applied. This might
  /// involve conflict checks, version matching etc.
  /// @param workspace_edit The edits to be validated.
  /// @return 'true' if edits are valid and can be applied; otherwise, 'false'.
  async fn validate_edits(&self, workspace_edit: &WorkspaceEdit) -> anyhow::Result<bool>;

  /// Applies a `WorkspaceEdit` to the workspace, effectively editing text documents
  /// and/or performing resource operations like file creation or renaming.
  /// @param edit The `ApplyWorkspaceEditParams` containing the edits to apply.
  /// @return `ApplyWorkspaceEditResponse` indicating the success or failure of the operation.
  async fn apply_edit(&mut self, edit: ApplyWorkspaceEditParams) -> anyhow::Result<ApplyWorkspaceEditResponse>;

  /// Applies a series of `TextEdit` objects to an open document. Ensures that
  /// all edits are applied atomically to avoid partial updates.
  /// @param document_identifier Identifies the document to edit, along with its version.
  /// @param edits The text edits to be applied.
  /// @return Success or failure of the operation.
  async fn apply_text_edits_to_document(
    &mut self,
    document_identifier: VersionedTextDocumentIdentifier,
    edits: Vec<TextEdit>,
  ) -> anyhow::Result<()>;

  /// Executes file system operations like creating, renaming, or deleting files and folders as specified
  /// by a `WorkspaceEdit`.
  /// @param operations The resource operations to execute.
  /// @return Success or failure of the operation.
  async fn execute_resource_operations(&mut self, operations: Vec<ResourceOp>) -> anyhow::Result<()>;

  /// Manages an undo/redo stack for workspace edits to allow reverting or reapplying changes.
  /// This requires maintaining a history of edits.
  /// @param workspace_edit The edit to potentially undo/redo.
  /// @param is_undo_operation 'true' if the operation is an undo; 'false' if it's a redo.
  /// @return Success or failure of managing the stack.
  async fn maintain_undo_redo_stack(
    &mut self,
    workspace_edit: &WorkspaceEdit,
    is_undo_operation: bool,
  ) -> anyhow::Result<()>;

  /// Applies multiple `WorkspaceEdit`s as a single transactional operation, ensuring all edits are applied
  /// atomically. Supports rollback if any part of the operation fails.
  /// @param workspace_edits The edits to apply as part of the transaction.
  /// @return Success or failure of the transaction.
  async fn apply_edits_as_transaction(&mut self, workspace_edits: Vec<WorkspaceEdit>) -> anyhow::Result<()>;
}

impl<T> LspEditor for T
where
  T: LspClient,
{
  /// Prepares a `WorkspaceEdit` object that encapsulates all intended modifications,
  /// such as computing diffs and determining necessary text edits, organized per document.
  /// @param parameters to define the changes.
  /// @return `WorkspaceEdit` with all prepared edits.
  async fn prepare_edits(&self /* parameters defining the changes */) -> anyhow::Result<WorkspaceEdit> {
    // DocumentChanges::Operations(Vec<DocumentChangeOperation>)
    todo!();
  }

  /// Validates the prepared edits to ensure they can be correctly applied. This might
  /// involve conflict checks, version matching etc.
  /// @param workspace_edit The edits to be validated.
  /// @return 'true' if edits are valid and can be applied; otherwise, 'false'.
  async fn validate_edits(&self, workspace_edit: &WorkspaceEdit) -> anyhow::Result<bool> {
    todo!();
  }

  /// Applies a `WorkspaceEdit` to the workspace, effectively editing text documents
  /// and/or performing resource operations like file creation or renaming.
  /// @param edit The `ApplyWorkspaceEditParams` containing the edits to apply.
  /// @return `ApplyWorkspaceEditResponse` indicating the success or failure of the operation.
  async fn apply_edit(&mut self, edit: ApplyWorkspaceEditParams) -> anyhow::Result<ApplyWorkspaceEditResponse> {
    match self.send_request("workspace/applyEdit", Some(edit), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "workspace/applyEdit" to context of error
        Err(anyhow!("failed to send workspace/applyEdit request: {}", err))
      },
    }
  }

  /// Applies a series of `TextEdit` objects to an open document. Ensures that
  /// all edits are applied atomically to avoid partial updates.
  /// @param document_identifier Identifies the document to edit, along with its version.
  /// @param edits The text edits to be applied.
  /// @return Success or failure of the operation.
  async fn apply_text_edits_to_document(
    &mut self,
    document_identifier: VersionedTextDocumentIdentifier,
    edits: Vec<TextEdit>,
  ) -> anyhow::Result<()> {
    todo!();
  }

  /// Executes file system operations like creating, renaming, or deleting files and folders as specified
  /// by a `WorkspaceEdit`.
  /// @param operations The resource operations to execute.
  /// @return Success or failure of the operation.
  async fn execute_resource_operations(&mut self, operations: Vec<ResourceOp>) -> anyhow::Result<()> {
    todo!();
  }

  /// Manages an undo/redo stack for workspace edits to allow reverting or reapplying changes.
  /// This requires maintaining a history of edits.
  /// @param workspace_edit The edit to potentially undo/redo.
  /// @param is_undo_operation 'true' if the operation is an undo; 'false' if it's a redo.
  /// @return Success or failure of managing the stack.
  async fn maintain_undo_redo_stack(
    &mut self,
    workspace_edit: &WorkspaceEdit,
    is_undo_operation: bool,
  ) -> anyhow::Result<()> {
    todo!();
  }

  /// Applies multiple `WorkspaceEdit`s as a single transactional operation, ensuring all edits are applied
  /// atomically. Supports rollback if any part of the operation fails.
  /// @param workspace_edits The edits to apply as part of the transaction.
  /// @return Success or failure of the transaction.
  async fn apply_edits_as_transaction(&mut self, workspace_edits: Vec<WorkspaceEdit>) -> anyhow::Result<()> {
    todo!();
  }
}
