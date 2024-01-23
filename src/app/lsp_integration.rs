use lsp_types::*;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;

#[rpc(client)]
pub trait LspApi {

  // Server lifecycle methods
  #[method(name = "initialize")]
  async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult, ErrorObjectOwned>;
  #[method(name = "shutdown")]
  async fn shutdown(&self) -> Result<(), ErrorObjectOwned>;
  // No need to return anything for the exit notification.
  #[method(name = "exit")]
  async fn exit(&self) -> Result<(), ErrorObjectOwned>;

  // Text synchronization methods
  #[method(name = "textDocument/didOpen")]
  async fn did_open(&self, params: DidOpenTextDocumentParams) -> Result<(), ErrorObjectOwned>;
  #[method(name = "textDocument/didChange")]
  async fn did_change(&self, params: DidChangeTextDocumentParams) -> Result<(), ErrorObjectOwned>;
  #[method(name = "textDocument/didClose")]
  async fn did_close(&self, params: DidCloseTextDocumentParams) -> Result<(), ErrorObjectOwned>;
  #[method(name = "textDocument/didSave")]
  async fn did_save(&self, params: DidSaveTextDocumentParams) -> Result<(), ErrorObjectOwned>;

  // Language features methods
  #[method(name = "textDocument/completion")]
  async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>, ErrorObjectOwned>;
  #[method(name = "textDocument/hover")]
  async fn hover(&self, params: HoverParams) -> Result<Option<Hover>, ErrorObjectOwned>;
  #[method(name = "textDocument/signatureHelp")]
  async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>, ErrorObjectOwned>;
  #[method(name = "textDocument/declaration")]
  async fn declaration(&self, params: DeclarationParams) -> Result<Option<GotoDefinitionResponse>, ErrorObjectOwned>;
  #[method(name = "textDocument/definition")]
  async fn definition(&self, params: DefinitionParams) -> Result<Option<GotoDefinitionResponse>, ErrorObjectOwned>;
  #[method(name = "textDocument/typeDefinition")]
  async fn type_definition(&self, params: TypeDefinitionParams) -> Result<Option<GotoTypeDefinitionResponse>, ErrorObjectOwned>;
  #[method(name = "textDocument/implementation")]
  async fn implementation(&self, params: ImplementationParams) -> Result<Option<GotoImplementationResponse>, ErrorObjectOwned>;
  #[method(name = "textDocument/references")]
  async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>, ErrorObjectOwned>;
  #[method(name = "textDocument/documentHighlight")]
  async fn document_highlight(&self, params: DocumentHighlightParams) -> Result<Option<Vec<DocumentHighlight>>, ErrorObjectOwned>;
  #[method(name = "textDocument/documentSymbol")]
  async fn document_symbol(&self, params: DocumentSymbolParams) -> Result<Option<DocumentSymbolResponse>, ErrorObjectOwned>;
  #[method(name = "textDocument/formatting")]
  async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>, ErrorObjectOwned>;
  #[method(name = "textDocument/rangeFormatting")]
  async fn range_formatting(&self, params: DocumentRangeFormattingParams) -> Result<Option<Vec<TextEdit>>, ErrorObjectOwned>;
  #[method(name = "textDocument/onTypeFormatting")]
  async fn on_type_formatting(&self, params: DocumentOnTypeFormattingParams) -> Result<Option<Vec<TextEdit>>, ErrorObjectOwned>;
  #[method(name = "textDocument/rename")]
  async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>, ErrorObjectOwned>;
  #[method(name = "textDocument/prepareRename")]
  async fn prepare_rename(&self, params: PrepareRenameParams) -> Result<Option<PrepareRenameResponse>, ErrorObjectOwned>;
  #[method(name = "textDocument/codeAction")]
  async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>, ErrorObjectOwned>;
  #[method(name = "textDocument/codeLens")]
  async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>, ErrorObjectOwned>;
  #[method(name = "codeLens/resolve")]
  async fn code_lens_resolve(&self, params: CodeLens) -> Result<CodeLens, ErrorObjectOwned>;
  #[method(name = "textDocument/documentLink")]
  async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>, ErrorObjectOwned>;
  #[method(name = "documentLink/resolve")]
  async fn document_link_resolve(&self, params: DocumentLink) -> Result<DocumentLink, ErrorObjectOwned>;
  #[method(name = "textDocument/documentColor")]
  async fn document_color(&self, params: DocumentColorParams) -> Result<Vec<ColorInformation>, ErrorObjectOwned>;
  #[method(name = "textDocument/colorPresentation")]
  async fn color_presentation(&self, params: ColorPresentationParams) -> Result<Vec<ColorPresentation>, ErrorObjectOwned>;
  #[method(name = "textDocument/foldingRange")]
  async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>, ErrorObjectOwned>;
  #[method(name = "textDocument/selectionRange")]
  async fn selection_range(&self, params: SelectionRangeParams) -> Result<Option<Vec<SelectionRange>>, ErrorObjectOwned>;
  #[method(name = "workspace/symbol")]
  async fn workspace_symbol(&self, params: WorkspaceSymbolParams) -> Result<Option<Vec<SymbolInformation>>, ErrorObjectOwned>;
  #[method(name = "workspace/executeCommand")]
  async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<ExecuteCommandResponse>, ErrorObjectOwned>;

  // Semantic Tokens
  #[method(name = "textDocument/semanticTokens/full")]
  async fn semantic_tokens_full(&self, params: SemanticTokensParams) -> Result<Option<SemanticTokensResult>, ErrorObjectOwned>;
  #[method(name = "textDocument/semanticTokens/full/delta")]
  async fn semantic_tokens_full_delta(
    &self,
    params: SemanticTokensDeltaParams,
  ) -> Result<Option<SemanticTokensFullDeltaResult>, ErrorObjectOwned>;
  #[method(name = "textDocument/semanticTokens/range")]
  async fn semantic_tokens_range(
    &self,
    params: SemanticTokensRangeParams,
  ) -> Result<Option<SemanticTokensResult>, ErrorObjectOwned>;
  #[method(name = "workspace/semanticTokens/refresh")]
  async fn semantic_tokens_refresh(&self) -> Result<(), ErrorObjectOwned>;

  // Linked Editing Range
  #[method(name = "textDocument/linkedEditingRange")]
  async fn linked_editing_range(&self, params: LinkedEditingRangeParams) -> Result<Option<LinkedEditingRanges>, ErrorObjectOwned>;

  // Moniker
  #[method(name = "textDocument/moniker")]
  async fn moniker(&self, params: MonikerParams) -> Result<Option<Vec<Moniker>>, ErrorObjectOwned>;

  // Call Hierarchy
  #[method(name = "textDocument/prepareCallHierarchy")]
  async fn prepare_call_hierarchy(
    &self,
    params: CallHierarchyPrepareParams,
  ) -> Result<Option<Vec<CallHierarchyItem>>, ErrorObjectOwned>;
  #[method(name = "callHierarchy/incomingCalls")]
  async fn call_hierarchy_incoming_calls(
    &self,
    params: CallHierarchyIncomingCallsParams,
  ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, ErrorObjectOwned>;
  #[method(name = "callHierarchy/outgoingCalls")]
  async fn call_hierarchy_outgoing_calls(
    &self,
    params: CallHierarchyOutgoingCallsParams,
  ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, ErrorObjectOwned>;

  // File Operations (proposed in 3.16 as part of workspace edits)
  #[method(name = "workspace/willCreateFiles")]
  async fn will_create_files(&self, params: CreateFilesParams) -> Result<WorkspaceEdit, ErrorObjectOwned>;
  #[method(name = "workspace/didCreateFiles")]
  async fn did_create_files(&self, params: CreateFilesParams) -> Result<(), ErrorObjectOwned>;
  #[method(name = "workspace/willRenameFiles")]
  async fn will_rename_files(&self, params: RenameFilesParams) -> Result<WorkspaceEdit, ErrorObjectOwned>;
  #[method(name = "workspace/didRenameFiles")]
  async fn did_rename_files(&self, params: RenameFilesParams) -> Result<(), ErrorObjectOwned>;
  #[method(name = "workspace/willDeleteFiles")]
  async fn will_delete_files(&self, params: DeleteFilesParams) -> Result<WorkspaceEdit, ErrorObjectOwned>;
  #[method(name = "workspace/didDeleteFiles")]
  async fn did_delete_files(&self, params: DeleteFilesParams) -> Result<(), ErrorObjectOwned>;

  // Work Done Progress
  #[method(name = "window/workDoneProgress/create")]
  async fn work_done_progress_create(&self, params: WorkDoneProgressCreateParams) -> Result<(), ErrorObjectOwned>;
  #[method(name = "window/workDoneProgress/cancel")]
  async fn work_done_progress_cancel(&self, params: WorkDoneProgressCancelParams) -> Result<(), ErrorObjectOwned>;
}
