use super::lsp_client::*;
use anyhow::anyhow;
use backoff::ExponentialBackoff;
use lsp_types::*;
use serde_json::{from_value, Value};

#[allow(async_fn_in_trait)]
pub trait LspNavigation
where
  Self: LspClient,
{
  async fn document_symbol(&mut self, params: DocumentSymbolParams) -> anyhow::Result<Option<DocumentSymbolResponse>>;
  async fn find_references(&mut self, params: ReferenceParams) -> anyhow::Result<Option<Vec<Location>>>;
  async fn rename(&mut self, params: RenameParams) -> anyhow::Result<Option<Value>>;
  async fn formatting(&mut self, params: DocumentFormattingParams) -> anyhow::Result<Option<Vec<TextEdit>>>;
  async fn range_formatting(&mut self, params: DocumentRangeFormattingParams) -> anyhow::Result<Option<Vec<TextEdit>>>;
  async fn completion(&mut self, params: CompletionParams) -> anyhow::Result<Option<CompletionResponse>>;
  async fn hover(&mut self, params: TextDocumentPositionParams) -> anyhow::Result<Option<Hover>>;
  async fn goto_declaration(
    &mut self,
    params: TextDocumentPositionParams,
  ) -> anyhow::Result<Option<GotoDefinitionResponse>>;
  async fn goto_definition(
    &mut self,
    params: TextDocumentPositionParams,
  ) -> anyhow::Result<Option<GotoDefinitionResponse>>;
  async fn goto_implementation(&mut self, params: TextDocumentPositionParams) -> anyhow::Result<Option<Location>>;
  async fn goto_type_definition(&mut self, params: TextDocumentPositionParams) -> anyhow::Result<Option<Location>>;
  async fn local_documentation(&mut self, params: TextDocumentPositionParams) -> anyhow::Result<Option<Hover>>;
  // fn apply_document_symbol_client_capabilities(&mut self) -> anyhow::Result<()> {
  //   self.update_capabilities(ClientCapabilities {
  //     text_document: Some(TextDocumentClientCapabilities {
  //       document_symbol: Some(DocumentSymbolClientCapabilities {
  //         dynamic_registration: None,
  //         symbol_kind: None,
  //         hierarchical_document_symbol_support: Some(true),
  //         tag_support: None,
  //       }),
  //       ..Default::default()
  //     }),
  //     ..Default::default()
  //   })
  // }
  // fn apply_workspace_symbol_client_capabilities(&mut self) -> anyhow::Result<()> {
  //   self.update_capabilities(ClientCapabilities {
  //     workspace: Some(WorkspaceClientCapabilities {
  //       symbol: Some(WorkspaceSymbolClientCapabilities {
  //         dynamic_registration: None,
  //         symbol_kind: None,
  //         tag_support: None,
  //         resolve_support: None,
  //       }),
  //       ..Default::default()
  //     }),
  //     ..Default::default()
  //   })
  // }
  // fn apply_find_references_client_capabilities(&mut self) -> anyhow::Result<()> {
  //   self.update_capabilities(ClientCapabilities {
  //     text_document: Some(TextDocumentClientCapabilities {
  //       references: Some(DynamicRegistrationClientCapabilities { dynamic_registration: Some(true) }),
  //       ..Default::default()
  //     }),
  //     ..Default::default()
  //   })
  // }
}

impl<T> LspNavigation for T
where
  T: LspClient,
{
  async fn document_symbol(&mut self, params: DocumentSymbolParams) -> anyhow::Result<Option<DocumentSymbolResponse>> {
    match self.send_request("textDocument/documentSymbol", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "textDocument/documentSymbol" to context of error
        Err(anyhow!("failed to send textDocument/documentSymbol request: {}", err))
      },
    }
  }

  async fn find_references(&mut self, params: ReferenceParams) -> anyhow::Result<Option<Vec<Location>>> {
    match self.send_request("textDocument/references", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "textDocument/references" to context of error
        Err(anyhow!("failed to send textDocument/references request: {}", err))
      },
    }
  }

  async fn rename(&mut self, params: RenameParams) -> anyhow::Result<Option<Value>> {
    match self.send_request("textDocument/rename", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "textDocument/rename" to context of error
        Err(anyhow!("failed to send textDocument/rename request: {}", err))
      },
    }
  }

  async fn formatting(&mut self, params: DocumentFormattingParams) -> anyhow::Result<Option<Vec<TextEdit>>> {
    match self.send_request("textDocument/formatting", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "textDocument/formatting" to context of error
        Err(anyhow!("failed to send textDocument/formatting request: {}", err))
      },
    }
  }

  async fn range_formatting(&mut self, params: DocumentRangeFormattingParams) -> anyhow::Result<Option<Vec<TextEdit>>> {
    match self.send_request("textDocument/rangeFormatting", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "textDocument/rangeFormatting" to context of error
        Err(anyhow!("failed to send textDocument/rangeFormatting request: {}", err))
      },
    }
  }

  async fn completion(&mut self, params: CompletionParams) -> anyhow::Result<Option<CompletionResponse>> {
    match self.send_request("textDocument/completion", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "textDocument/completion" to context of error
        Err(anyhow!("failed to send textDocument/completion request: {}", err))
      },
    }
  }

  async fn hover(&mut self, params: TextDocumentPositionParams) -> anyhow::Result<Option<Hover>> {
    match self.send_request("textDocument/hover", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "textDocument/hover" to context of error
        Err(anyhow!("failed to send textDocument/hover request: {}", err))
      },
    }
  }

  async fn goto_declaration(
    &mut self,
    params: TextDocumentPositionParams,
  ) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    match self.send_request("textDocument/declaration", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "textDocument/declaration" to context of error
        Err(anyhow!("failed to send textDocument/declaration request: {}", err))
      },
    }
  }

  async fn goto_definition(
    &mut self,
    params: TextDocumentPositionParams,
  ) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    match self.send_request("textDocument/definition", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "textDocument/definition" to context of error
        Err(anyhow!("failed to send textDocument/definition request: {}", err))
      },
    }
  }

  async fn goto_implementation(&mut self, params: TextDocumentPositionParams) -> anyhow::Result<Option<Location>> {
    match self.send_request("textDocument/implementation", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "textDocument/implementation" to context of error
        Err(anyhow!("failed to send textDocument/implementation request: {}", err))
      },
    }
  }

  async fn goto_type_definition(&mut self, params: TextDocumentPositionParams) -> anyhow::Result<Option<Location>> {
    match self.send_request("textDocument/typeDefinition", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => Err(anyhow!("failed to send textDocument/typeDefinition request: {}", err)),
    }
  }

  async fn local_documentation(&mut self, params: TextDocumentPositionParams) -> anyhow::Result<Option<Hover>> {
    match self.send_request("textDocument/hover", Some(params), self.next_id()).await {
      Ok(result) => from_value(result).map_err(Into::into),
      Err(err) => {
        // add "textDocument/hover" to context of error
        Err(anyhow!("failed to send textDocument/hover request: {}", err))
      },
    }
  }
}
