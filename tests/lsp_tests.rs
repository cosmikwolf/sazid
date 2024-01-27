// tests/lsp_client.rs
use lsp_types::*;
use ntest::timeout;
use sazid::app::lsp::{lsp_client::LspClient, lsp_stdio::LspClientStdio};

// The actual test function
#[tokio::test]
#[timeout(3000)]
async fn test_rust_analyzer_connection() -> anyhow::Result<()> {
  // Create an LspClientStdio instance
  let mut lsp_client = LspClientStdio::create().await?;

  let test_project_path = std::env::current_dir()?.join("tests/assets/testproject");

  // a json value that is parsed from a raw string literal
  let s = r#"
  { "cargo": { "buildScripts": { "enable": true } }, "procMacro": { "enable": true } }
  "#;
  let initialization_options = Some(serde_json::from_str(s).unwrap());
  // .split('\n')
  // .collect::<String>();
  let capabilities = ClientCapabilities {
    workspace: Some(WorkspaceClientCapabilities {
      workspace_folders: Some(WorkspaceFolderCapability {
        supported: Some(true),
        change_notifications: Some(OneOf::Left(true)),
      }),
      ..Default::default()
    }),
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

  let process_id = Some((std::process::id() as u64).try_into().unwrap());
  // Send `initialize` request to rust-analyzer
  let init_params = InitializeParams {
    client_info: None,
    locale: None,
    work_done_progress_params: Default::default(),
    process_id,
    root_path: None,
    root_uri: None,
    initialization_options,
    capabilities,
    trace: None,
    workspace_folders: None,
  };
  // println!("init_params: {:#?}", init_params);

  let init_result = lsp_client.initialize(init_params).await?;
  // println!("init_result: {:#?}", init_result);

  assert!(init_result.capabilities.text_document_sync.is_some());
  // println!("initiaize_result: {:#?}", init_result);
  let initiaized_result = lsp_client.initialized().await;
  assert!(initiaized_result.is_ok());
  // println!("initiaized_result: {:#?}", initiaized_result);
  // Open a dummy file
  let did_open_params = DidOpenTextDocumentParams {
    text_document: TextDocumentItem {
      uri: "file:///dummy.rs".parse()?,
      language_id: "rust".into(),
      version: 0,
      text: "fn main() {}".into(),
    },
  };
  // println!("did_open_params: {:#?}", did_open_params);

  let did_open_params_result = lsp_client.did_open(did_open_params).await;
  assert!(did_open_params_result.is_ok());

  // Send `shutdown` request to rust-analyzer
  let shutdown_result = lsp_client.shutdown().await;
  assert!(shutdown_result.is_ok());

  Ok(())
}
