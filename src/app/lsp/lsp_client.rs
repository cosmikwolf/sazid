use futures_util::Future;
use lsp_types::*;
use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

use super::lsp_stdio::StdioBuffers;

pub struct State {
  pub sequence: AtomicU64,
}

pub trait LspClient
where
  Self: std::marker::Sized,
{
  // async fn perform_action_with_retry<F, R, Fut>(&self, action: F) -> anyhow::Result<R>
  // where
  //   F: Fn(Arc<Mutex<State>>, String) -> Fut,
  //   Fut: Future<Output = anyhow::Result<R>>;
  async fn handle_server_messages(receiver: mpsc::Receiver<String>, state: Arc<Mutex<State>>);
  fn next_id(&self) -> u64;
  fn update_capabilities(&mut self, new_capabilities: ClientCapabilities) -> anyhow::Result<()>;
  async fn get_stdio_handle<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut StdioBuffers) -> T + Send,
    T: Send;
  async fn create() -> anyhow::Result<Self>;
  async fn initialize(&mut self, initialize_params: InitializeParams) -> anyhow::Result<InitializeResult>;
  async fn initialized(&mut self) -> anyhow::Result<()>;
  async fn shutdown(&mut self) -> anyhow::Result<()>;
  async fn send_request<T: Serialize>(&mut self, method: &str, params: Option<T>, id: u64) -> anyhow::Result<Value>;
  async fn send_notification<T: Serialize>(&mut self, method: &str, params: T) -> anyhow::Result<()>;
  async fn workspace_symbol_query(&mut self, query: &str) -> anyhow::Result<Option<Vec<SymbolInformation>>>;
  async fn start_work_progress_loop(&mut self) -> anyhow::Result<()>;
  async fn end_work_progress_loop(&mut self) -> anyhow::Result<()>;
  async fn check_work_token_progress(&mut self, token: ProgressToken) -> anyhow::Result<WorkDoneProgressReport>;

  // fn apply_client_capabilities(&mut self);
  // async fn did_open(&mut self, params: DidOpenTextDocumentParams) -> anyhow::Result<()>;
  // async fn did_change(&mut self, params: DidChangeTextDocumentParams) -> anyhow::Result<()>;
  // async fn did_save(&mut self, params: DidSaveTextDocumentParams) -> anyhow::Result<()>;
  // async fn did_close(&mut self, params: DidCloseTextDocumentParams) -> anyhow::Result<()>;
}
