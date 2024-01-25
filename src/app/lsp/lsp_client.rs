use async_trait::async_trait;
use lsp_types::{DidOpenTextDocumentParams, InitializeParams, InitializeResult};

#[async_trait]
pub trait LspClient {
  async fn initialize(&mut self, initialize_params: InitializeParams) -> anyhow::Result<InitializeResult>;
  async fn shutdown(&mut self) -> anyhow::Result<()>;
  async fn did_open(&mut self, params: DidOpenTextDocumentParams) -> anyhow::Result<()>;
}
