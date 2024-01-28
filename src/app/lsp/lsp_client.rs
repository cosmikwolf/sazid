
#[allow(async_fn_in_trait)]
pub trait LspClient
where
  Self: std::marker::Sized,
{
  fn next_id(&self) -> u64;
  fn update_capabilities(&mut self, new_capabilities: ClientCapabilities) -> anyhow::Result<()>;

  async fn create() -> anyhow::Result<Self>;
  async fn initialize(&mut self, initialize_params: InitializeParams) -> anyhow::Result<InitializeResult>;
  async fn initialized(&mut self) -> anyhow::Result<()>;
  async fn shutdown(&mut self) -> anyhow::Result<()>;
  async fn send_request<T: Serialize>(&mut self, method: &str, params: Option<T>, id: u64) -> anyhow::Result<Value>;
  async fn send_notification<T: Serialize>(&mut self, method: &str, params: T) -> anyhow::Result<()>;
  async fn read_error(&mut self) -> anyhow::Result<()>;
  async fn read_response(&mut self, expected_id: u64) -> anyhow::Result<Value>;
  // fn apply_client_capabilities(&mut self);
  // async fn did_open(&mut self, params: DidOpenTextDocumentParams) -> anyhow::Result<()>;
  // async fn did_change(&mut self, params: DidChangeTextDocumentParams) -> anyhow::Result<()>;
  // async fn did_save(&mut self, params: DidSaveTextDocumentParams) -> anyhow::Result<()>;
  // async fn did_close(&mut self, params: DidCloseTextDocumentParams) -> anyhow::Result<()>;
}
