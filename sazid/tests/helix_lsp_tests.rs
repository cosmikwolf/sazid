use std::path::Path;
use std::str::from_utf8;
use std::sync::{Arc, Mutex};

// tests/lsp_client.rs
use helix_core;
use helix_core::config::default_syntax_loader;
use helix_core::syntax::Loader;
use helix_loader;
use helix_loader::grammar::{get_language, load_runtime_file};
use helix_lsp::jsonrpc::{MethodCall, Notification};
use helix_lsp::{self, Call, LspProgressMap, ProgressStatus, Registry};
use lsp_types::notification::Progress;
use lsp_types::*;
use tokio::time;
use tokio::time::Duration;

// The actual test function

pub fn test_lang_config() -> helix_core::syntax::Configuration {
  let default_config = include_bytes!("./assets/languages_test.toml");
  toml::from_str::<helix_core::syntax::Configuration>(from_utf8(default_config).unwrap())
    .expect("Could not parse built-in languages.toml to valid toml")
}

#[tokio::test]
async fn test_rust_analyzer_connection() -> anyhow::Result<()> {
  let test_project_path = std::env::current_dir().unwrap().join("tests/assets/testproject");
  std::env::set_current_dir(&test_project_path).unwrap();
  assert!(test_project_path.exists());
  let workspace_folders = Some(vec![WorkspaceFolder {
    uri: url::Url::from_directory_path(test_project_path.clone()).unwrap(),
    name: "testproject".to_string(),
  }]);

  let config = default_syntax_loader();
  let config = test_lang_config();
  let loader = Loader::new(config);

  let rust_lang_config = Arc::clone(&loader.language_config_for_name("rust").unwrap());
  println!("Rust lang config: {:#?}", rust_lang_config);

  let toml_lang_config = Arc::clone(&loader.language_config_for_file_name(Path::new("Cargo.toml")).unwrap());

  let root_dirs = vec![test_project_path.clone()];
  // println!("Root dirs: {:?}", root_dirs);
  let mut rust_registry = Registry::new(Arc::new(loader));
  let rust_client = rust_registry
    .get(&rust_lang_config, None, root_dirs.as_slice(), true)
    .find(|(name, client_res)| name == "rust-analyzer")
    .unwrap()
    .1
    .unwrap();

  while !rust_client.is_initialized() {
    // wait 10ms
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
  }

  let mut lsp_progress = LspProgressMap::new();

  let work_done_token = ProgressToken::String("rustAnalyzer/Fetching".to_string());
  lsp_progress.create(rust_client.id(), work_done_token.clone());

  let src = WorkspaceFolder {
    name: "src".to_string(),
    uri: url::Url::from_directory_path(test_project_path.clone().join("src")).unwrap(),
  };
  use futures_util::StreamExt;
  let rust_workspace_symbols_response = rust_client.workspace_symbols("".to_string());
  if let Some(symbols) = rust_workspace_symbols_response {
    println!("Rust Workspace symbols: {:#?}", symbols.await);
  } else {
    println!("No symbols found");
  }
  let timeout = Duration::from_secs(2);
  if let Err(_) = time::timeout(timeout, async {
    loop {
      if let Some((size, message)) = rust_registry.incoming.next().await {
        match message {
          Call::MethodCall(MethodCall { method, params, id, .. }) => {
            let reply = match MethodCall::parse(&method, params) {
              Err(helix_lsp::Error::Unhandled) => {
                error!("Language Server: Method {} not found in request {}", method, id);
                Err(helix_lsp::jsonrpc::Error {
                  code: helix_lsp::jsonrpc::ErrorCode::MethodNotFound,
                  message: format!("Method not found: {}", method),
                  data: None,
                })
              },
              Err(err) => {
                log::error!("Language Server: Received malformed method call {} in request {}: {}", method, id, err);
                Err(helix_lsp::jsonrpc::Error {
                  code: helix_lsp::jsonrpc::ErrorCode::ParseError,
                  message: format!("Malformed method call: {}", method),
                  data: None,
                })
              },
              Ok(MethodCall::WorkDoneProgressCreate(params)) => {
                lsp_progress.create(server_id, params.token);
                println!("Progress token created: {:#?}", params);

                // let spinner = editor_view.spinners_mut().get_or_create(server_id);
                // if spinner.is_stopped() {
                //   spinner.start();
                // }

                Ok(serde_json::Value::Null)
              },
            };
          },
          Call::Notification(helix_lsp::jsonrpc::Notification { method, params, .. }) => {
            let notification = match Notification::parse(&method, params) {
              Ok(notification) => notification,
              Err(helix_lsp::Error::Unhandled) => {
                info!("Ignoring Unhandled notification from Language Server");
                return;
              },
              Err(err) => {
                error!("Ignoring unknown notification from Language Server: {}", err);
                return;
              },
            };

            match notification {
              Notification::ProgressMessage(params) => {
                let ProgressParams { token, value } = params;

                let ProgressParamsValue::WorkDone(work) = value;
                let parts = match &work {
                  WorkDoneProgress::Begin(WorkDoneProgressBegin { title, message, percentage, .. }) => {
                    (Some(title), message, percentage)
                  },
                  WorkDoneProgress::Report(WorkDoneProgressReport { message, percentage, .. }) => {
                    (None, message, percentage)
                  },
                  WorkDoneProgress::End(WorkDoneProgressEnd { message }) => {
                    if message.is_some() {
                      (None, message, &None)
                    } else {
                      lsp_progress.end_progress(server_id, &token);
                      // if !lsp_progress.is_progressing(server_id) {
                      //   editor_view.spinners_mut().get_or_create(server_id).stop();
                      // }
                      // self.editor.clear_status();

                      // we want to render to clear any leftover spinners or messages
                      return;
                    }
                  },
                };

                let token_d: &dyn std::fmt::Display = match &token {
                  NumberOrString::Number(n) => n,
                  NumberOrString::String(s) => s,
                };

                let status = match parts {
                  (Some(title), Some(message), Some(percentage)) => {
                    format!("[{}] {}% {} - {}", token_d, percentage, title, message)
                  },
                  (Some(title), None, Some(percentage)) => {
                    format!("[{}] {}% {}", token_d, percentage, title)
                  },
                  (Some(title), Some(message), None) => {
                    format!("[{}] {} - {}", token_d, title, message)
                  },
                  (None, Some(message), Some(percentage)) => {
                    format!("[{}] {}% {}", token_d, percentage, message)
                  },
                  (Some(title), None, None) => {
                    format!("[{}] {}", token_d, title)
                  },
                  (None, Some(message), None) => {
                    format!("[{}] {}", token_d, message)
                  },
                  (None, None, Some(percentage)) => {
                    format!("[{}] {}%", token_d, percentage)
                  },
                  (None, None, None) => format!("[{}]", token_d),
                };

                if let WorkDoneProgress::End(_) = work {
                  lsp_progress.end_progress(server_id, &token);
                  // if !self.lsp_progress.is_progressing(server_id) {
                  //   editor_view.spinners_mut().get_or_create(server_id).stop();
                  // }
                } else {
                  lsp_progress.update(server_id, token, work);
                }

                // if self.config.load().editor.lsp.display_messages {
                //   self.editor.set_status(status);
                // }
              },
              Call::Notification(Notification::ProgressMessage(_params)) => {
                // do nothing
              },
              Call::Notification(Notification::Exit) => {
                // self.editor.set_status("Language server exited");

                // LSPs may produce diagnostics for files that haven't been opened in helix,
                // we need to clear those and remove the entries from the list if this leads to
                // an empty diagnostic list for said files
                // for diags in self.editor.diagnostics.values_mut() {
                //   diags.retain(|(_, lsp_id)| *lsp_id != server_id);
                // }
                //
                // self.editor.diagnostics.retain(|_, diags| !diags.is_empty());
                //
                // // Clear any diagnostics for documents with this server open.
                // for doc in self.editor.documents_mut() {
                //   doc.clear_diagnostics(Some(server_id));
                // }
                //
                // // Remove the language server from the registry.
                // self.editor.language_servers.remove_by_id(server_id);
              },
            }
          },
          _ => {
            println!("Unexpected message");
          },
        }
      }
    }
    while let Some(progress) = lsp_progress.progress(1, &work_done_token) {
      use futures_util::StreamExt;
      match progress.progress() {
        Some(WorkDoneProgress::Begin(begin)) => {
          println!("Progress Begin: {:#?}", begin);
        },
        Some(WorkDoneProgress::Report(report)) => {
          println!("Progress Report: {:#?}", report);
        },
        Some(WorkDoneProgress::End(end)) => {
          println!("Progress End: {:#?}", end);
          break;
        },
        None => {
          println!("No progress token found");
        },
      }
    }
    true;
  })
  .await
  {
    // The test has timed out
    println!("Test timed out");
  }
  panic!();
  // rust_client.did_change_workspace(vec![src], vec![]).await?;

  // println!("Workspace folders: {:#?}", workspace_folders);

  tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
  let rust_workspace_symbols_response = rust_client.workspace_symbols("".to_string());
  if let Some(symbols) = rust_workspace_symbols_response {
    println!("Rust Workspace symbols: {:#?}", symbols.await);
    panic!();
  } else {
    println!("No symbols found");
    panic!();
  }

  let toml_client = rust_registry
    .get(&toml_lang_config, None, root_dirs.as_slice(), true)
    .find(|(name, client_res)| name == "taplo")
    .unwrap()
    .1
    .unwrap();

  while !toml_client.is_initialized() {
    // wait 10ms
    println!("Waiting for toml client to initialize");
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
  }

  let toml_workspace_symbols_response = toml_client.workspace_symbols("".to_string());
  if let Some(symbols) = toml_workspace_symbols_response {
    println!("Toml Workspace symbols: {:#?}", symbols.await);
  } else {
    println!("No symbols found");
  }
  // Create an LspClientStdio instance
  Ok(())
}
