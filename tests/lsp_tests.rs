// tests/lsp_client.rs
use lsp_types::{
  ClientCapabilities, ClientInfo, DidOpenTextDocumentParams, GeneralClientCapabilities, InitializeParams,
  TextDocumentClientCapabilities, TextDocumentItem, TextDocumentSyncCapability, TextDocumentSyncClientCapabilities,
  WorkspaceFolder,
};
use sazid::app::lsp::lsp_client::LspClient;
use sazid::app::lsp::lsp_stdio::LspClientStdio;
use serde_json::json;

// The actual test function
#[tokio::test]
async fn test_rust_analyzer_connection() -> anyhow::Result<()> {
  // Create an LspClientStdio instance
  let mut lsp_client = LspClientStdio::create().await?;

  let test_project_path = std::env::current_dir()?.join("tests/assets/testproject");

  // a json value that is parsed from a raw string literal
  let s = r#"
  { "cargo": { "buildScripts": { "enable": true } }, "procMacro": { "enable": true } }
  "#;
  // let initialization_options = Some(serde_json::from_str(s).unwrap());
  // .split('\n')
  // .collect::<String>();
  let capabilities = ClientCapabilities {
    text_document: Some(TextDocumentClientCapabilities {
      synchronization: Some(TextDocumentSyncClientCapabilities { did_save: Some(true), ..Default::default() }),
      ..Default::default()
    }),
    ..Default::default()
  };
  let workspace_folders = Some(vec![WorkspaceFolder {
    uri: url::Url::from_directory_path(test_project_path.clone()).unwrap(),
    name: "testproject".to_string(),
  }]);
  let root_uri = Some(url::Url::from_directory_path(test_project_path).unwrap());
  let client_info = Some(ClientInfo { name: "sazid".to_string(), version: None });

  // let process_id = Some((std::process::id() as u64).try_into().unwrap());
  // Send `initialize` request to rust-analyzer
  let init_params = InitializeParams {
    client_info: None,
    locale: None,
    work_done_progress_params: Default::default(),
    process_id: None,
    root_path: None,
    root_uri: None,
    initialization_options: None,
    capabilities: ClientCapabilities::default(),
    trace: None,
    workspace_folders: None,
  };
  // println!("init_params: {:#?}", init_params);

  let init_result = lsp_client.initialize(init_params).await?;
  assert!(init_result.capabilities.text_document_sync.is_some());

  // Open a dummy file
  let did_open_params = DidOpenTextDocumentParams {
    text_document: TextDocumentItem {
      uri: "file:///dummy.rs".parse()?,
      language_id: "rust".into(),
      version: 0,
      text: "fn main() {}".into(),
    },
  };

  lsp_client.did_open(did_open_params).await?;

  // Send `shutdown` request to rust-analyzer
  lsp_client.shutdown().await?;

  Ok(())
}
