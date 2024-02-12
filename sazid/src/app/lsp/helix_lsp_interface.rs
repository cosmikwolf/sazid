use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use futures_util::TryFutureExt;
use helix_core::config::default_syntax_loader;
use helix_core::diagnostic::Severity;
use helix_core::syntax::Configuration;
use helix_core::syntax::LanguageConfiguration;
use helix_core::syntax::Loader;
use helix_lsp::lsp::{self, notification::Notification};
use helix_lsp::Client;
use helix_lsp::LspProgressMap;
use helix_lsp::ProgressStatus;
use helix_lsp::Registry;
use log::{debug, error, info, warn};
use lsp::DocumentSymbol;
use lsp::WorkDoneProgress;
use lsp::WorkDoneProgressEnd;
use serde_json::from_value;
use serde_json::json;
use url::Url;

use super::symbol_types::Workspace;

pub struct LanguageServerInterface {
  pub lsp_progress: LspProgressMap,
  pub language_servers: helix_lsp::Registry,
  loader: Arc<Loader>,
  pub status_msg: Option<(Cow<'static, str>, Severity)>,
}

impl LanguageServerInterface {
  pub fn new(config: Option<Configuration>) -> Self {
    let loader = match config {
      Some(config) => Arc::new(Loader::new(config)),
      None => Arc::new(Loader::new(default_syntax_loader())),
    };
    Self {
      lsp_progress: LspProgressMap::new(),
      loader: loader.clone(),
      language_servers: Registry::new(loader),
      status_msg: None,
    }
  }

  pub fn create_workspace(
    &mut self,
    workspace_path: PathBuf,
    language_name: &str,
    languge_server_name: &str,
    doc_path: Option<&PathBuf>,
  ) -> anyhow::Result<Workspace> {
    let root_dirs = &[workspace_path.clone()];
    let enable_snippets = false;
    let language_server =
      self.initialize_client(language_name, languge_server_name, doc_path, root_dirs, enable_snippets)?;
    let language_config =
      self.language_configuration_by_name(language_name).expect("can't find language configuration");
    Ok(Workspace::new(workspace_path, language_server.expect("unable to initialize language server"), language_config))
  }

  pub async fn get_semantic_tokens(&mut self, doc_url: &Url, id: usize) -> anyhow::Result<lsp::SemanticTokensResult> {
    let language_server = self.language_server_by_id(id).unwrap();
    let doc_id = lsp::TextDocumentIdentifier::new(doc_url.clone());
    if let Some(s) = language_server.semantic_tokens(doc_id.clone()) {
      let tokens = s.await.unwrap();
      let response: Option<lsp::SemanticTokensResult> = serde_json::from_value(tokens)?;
      let tokens = match response {
        Some(tokens) => tokens,
        None => return Err(anyhow::anyhow!("no semantic tokens found")),
      };
      Ok(tokens)
    } else {
      Err(anyhow::anyhow!("no semantic tokens found"))
    }
  }

  pub async fn query_workspace_symbols(
    &mut self,
    query: &str,
    ids: &[usize],
  ) -> anyhow::Result<Vec<lsp::WorkspaceSymbol>> {
    match self.wait_for_progress_token_completion(ids).await {
      Ok(_) => {
        let mut results = vec![];
        for client in self.language_servers.iter_clients() {
          println!("client id: {}", client.id());

          if ids.contains(&client.id()) {
            println!("client name is included: {}", client.name());

            if let Some(s) = client.workspace_symbols(query.into()) {
              let symbols = s.await.unwrap();
              results
                .extend(from_value::<Vec<lsp::WorkspaceSymbol>>(symbols).expect("failed to parse workspace symbols"))
            }
          }
        }
        Ok(results)
      },
      Err(e) => Err(e),
    }
  }

  async fn get_workspace_files(&mut self, id: usize) -> anyhow::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = Vec::new();

    match self.language_server_by_id(id) {
      Some(language_server) => {
        let workspace_folders = language_server.workspace_folders();
        let wf = workspace_folders.await;
        for folder in wf.iter() {
          let folderfiles = walkdir::WalkDir::new(folder.uri.to_file_path().unwrap())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .filter(|e| e.path().extension().unwrap_or_default() == "rs")
            .flat_map(|e| e.path().canonicalize())
            .collect::<Vec<PathBuf>>();
          files.extend(folderfiles);
        }
      },
      None => return Err(anyhow::anyhow!("no language server with id found")),
    }
    println!("files: {:#?}", files);
    Ok(files)
  }

  pub async fn get_workspace_document_symbols(&mut self, id: usize) -> anyhow::Result<Vec<DocumentSymbol>> {
    log::debug!("get_workspace_document_symbols: {:#?}", id);
    let files = self.get_workspace_files(id).await?;
    let mut doc_symbols = vec![];
    for file in files.iter() {
      let uri = Url::from_file_path(file).unwrap();
      log::debug!("uri: {:#?}", uri);
      let symbols = self.query_document_symbols(&uri, &[id]).await.unwrap();
      doc_symbols.extend(symbols);
    }
    Ok(doc_symbols)
  }

  pub async fn query_document_symbols(&mut self, doc_url: &Url, ids: &[usize]) -> anyhow::Result<Vec<DocumentSymbol>> {
    match self.wait_for_progress_token_completion(ids).await {
      Ok(_) => {
        let mut results = vec![];
        for language_server in self.language_servers.iter_clients() {
          if ids.contains(&language_server.id()) {
            let doc_id = lsp::TextDocumentIdentifier::new(doc_url.clone());

            let _offset_encoding = language_server.offset_encoding();
            if let Some(s) = language_server.document_symbols(doc_id.clone()) {
              let symbols = s.await.unwrap();
              let response: Option<lsp::DocumentSymbolResponse> = serde_json::from_value(symbols)?;

              let symbols = match response {
                Some(symbols) => symbols,
                None => return anyhow::Ok(vec![]),
              };
              let symbols = match symbols {
                lsp::DocumentSymbolResponse::Nested(symbols) => {
                  symbols
                  // let mut flat_symbols = Vec::new();
                  // for symbol in symbols {
                  //   nested_to_flat(&mut flat_symbols, &doc_id, symbol, offset_encoding)
                  // }
                  // flat_symbols
                },
                lsp::DocumentSymbolResponse::Flat(_symbols) => {
                  // symbols.into_iter().map(|symbol| SymbolInformationItem { symbol, offset_encoding }).collect()
                  return Err(anyhow::anyhow!("document symbol support is required"));
                },
              };
              results.extend(symbols);
            }
          }
        }
        Ok(results)
      },
      Err(e) => Err(e),
    }
  }

  // check if all progress tokens are complete for language server with id
  pub fn progress_tokens_complete(&self, id: usize) -> Option<bool> {
    if let Some(prog_map) = self.lsp_progress.progress_map(id) {
      // prog_map.iter().for_each(|(k, v)| {
      //   println!("Progress: {:#?} - {:#?}", k, v);
      // });

      if prog_map
        .iter()
        .all(|(_k, v)| matches!(v, ProgressStatus::Started(WorkDoneProgress::End(WorkDoneProgressEnd { message: _ }))))
      {
        Some(true)
      } else {
        Some(false)
      }
    } else {
      None
    }
  }

  pub async fn wait_for_progress_token_completion(&mut self, ids: &[usize]) -> anyhow::Result<()> {
    loop {
      if ids.iter().all(|id| self.progress_tokens_complete(*id) == Some(true)) {
        break;
      } else {
        self.poll_language_server_events().await;
        let active_clients = self
          .language_servers
          .iter_clients()
          .filter(|client| ids.contains(&client.id()))
          .collect::<Vec<&Arc<Client>>>();

        if active_clients.is_empty() {
          return Err(anyhow::anyhow!("no language servers with matching ids found"));
        } else if active_clients.iter().all(|client| self.progress_tokens_complete(client.id()) == Some(true)) {
          break;
        }
      }
    }
    Ok(())
  }

  pub async fn poll_language_server_events(&mut self) {
    use futures_util::StreamExt;

    tokio::select! {
      biased;
     Some((size, message)) = self.language_servers.incoming.next() => {
      self.handle_language_server_message(message, size).await
     }
    }
  }

  pub fn initialize_client(
    &mut self,
    language_name: &str,
    languge_server_name: &str,
    doc_path: Option<&PathBuf>,
    root_dirs: &[PathBuf],
    enable_snippets: bool,
  ) -> Result<Option<Arc<Client>>, anyhow::Error> {
    match self.language_configuration_by_name(language_name) {
      Some(language_config) => {
        Ok(Some(
          self
            .language_servers
            .get(
              //
              &language_config,
              doc_path,
              root_dirs,
              enable_snippets,
            )
            .find(|(name, _client)| name == languge_server_name)
            .unwrap()
            .1
            .map_err(|e| anyhow::anyhow!(e))?,
        ))
      },
      None => Ok(None),
    }
  }

  pub fn language_configuration_by_name(&self, name: &str) -> Option<Arc<LanguageConfiguration>> {
    self.loader.language_config_for_name(name)
  }

  pub fn language_server_by_name(&self, language_server_name: &str) -> Option<&helix_lsp::Client> {
    println!("language_servers: {:?}", self.language_servers.iter_clients().count());

    self
      .language_servers
      .iter_clients()
      .find(|client| {
        println!("client name: {}", client.name());
        client.name() == language_server_name
      })
      .map(|client| &**client)
  }

  pub fn language_server_by_id(&self, language_server_id: usize) -> Option<&helix_lsp::Client> {
    self.language_servers.get_by_id(language_server_id)
  }

  pub async fn handle_language_server_message(&mut self, call: helix_lsp::Call, server_id: usize) {
    use helix_lsp::{Call, MethodCall, Notification};

    macro_rules! language_server {
      () => {
        match self.language_server_by_id(server_id) {
          Some(language_server) => language_server,
          None => {
            warn!("can't find language server with id `{}`", server_id);
            return;
          },
        }
      };
    }

    match call {
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
          Notification::Initialized => {
            let language_server = language_server!();

            // Trigger a workspace/didChangeConfiguration notification after initialization.
            // This might not be required by the spec but Neovim does this as well, so it's
            // probably a good idea for compatibility.
            if let Some(config) = language_server.config() {
              tokio::spawn(language_server.did_change_configuration(config.clone()));
            }

            // let docs = self.editor.documents().filter(|doc| doc.supports_language_server(server_id));
            //
            // // trigger textDocument/didOpen for docs that are already open
            // for doc in docs {
            //   let url = match doc.url() {
            //     Some(url) => url,
            //     None => continue, // skip documents with no path
            //   };
            //
            //   let language_id = doc.language_id().map(ToOwned::to_owned).unwrap_or_default();
            //
            //   tokio::spawn(language_server.text_document_did_open(url, doc.version(), doc.text(), language_id));
            // }
          },
          Notification::PublishDiagnostics(params) => {
            log::warn!("need to handle publish diagnostics: {:?}", params);
          },
          Notification::ShowMessage(params) => {
            log::warn!("unhandled window/showMessage: {:?}", params);
          },
          Notification::LogMessage(params) => {
            log::info!("window/logMessage: {:?}", params);
          },
          Notification::ProgressMessage(params) => {
            let lsp::ProgressParams { token, value } = params;

            let lsp::ProgressParamsValue::WorkDone(work) = value;
            let parts = match &work {
              lsp::WorkDoneProgress::Begin(lsp::WorkDoneProgressBegin { title, message, percentage, .. }) => {
                (Some(title), message, percentage)
              },
              lsp::WorkDoneProgress::Report(lsp::WorkDoneProgressReport { message, percentage, .. }) => {
                (None, message, percentage)
              },
              lsp::WorkDoneProgress::End(lsp::WorkDoneProgressEnd { message }) => {
                // if message.is_some() {
                (None, message, &None)
                // } else {
                // self.lsp_progress.end_progress(server_id, &token);
                // if !self.lsp_progress.is_progressing(server_id) {
                // editor_view.spinners_mut().get_or_create(server_id).stop();
                // }
                // self.clear_status();

                // we want to render to clear any leftover spinners or messages
                // return;
                // }
              },
            };

            let token_d: &dyn std::fmt::Display = match &token {
              lsp::NumberOrString::Number(n) => n,
              lsp::NumberOrString::String(s) => s,
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

            // if let lsp::WorkDoneProgress::End(_) = work {
            //   self.lsp_progress.end_progress(server_id, &token);
            //   if !self.lsp_progress.is_progressing(server_id) {
            //     // editor_view.spinners_mut().get_or_create(server_id).stop();
            //   }
            // } else {
            //   self.lsp_progress.update(server_id, token, work);
            // }
            //
            self.lsp_progress.update(server_id, token, work);
            // if self.config.load().editor.lsp.display_messages {
            self.set_status(status);
            // }
          },
          Notification::ProgressMessage(_params) => {
            // do nothing
          },
          Notification::Exit => {
            self.set_status("Language server exited");

            // LSPs may produce diagnostics for files that haven't been opened in helix,
            // we need to clear those and remove the entries from the list if this leads to
            // an empty diagnostic list for said files
            // for diags in self.editor.diagnostics.values_mut() {
            //   diags.retain(|(_, lsp_id)| *lsp_id != server_id);
            // }

            // self.editor.diagnostics.retain(|_, diags| !diags.is_empty());

            // Clear any diagnostics for documents with this server open.
            // for doc in self.editor.documents_mut() {
            //   doc.clear_diagnostics(Some(server_id));
            // }

            // Remove the language server from the registry.
            self.language_servers.remove_by_id(server_id);
          },
        }
      },
      Call::MethodCall(helix_lsp::jsonrpc::MethodCall { method, params, id, .. }) => {
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
            self.lsp_progress.create(server_id, params.token);

            // let editor_view = self.compositor.find::<ui::EditorView>().expect("expected at least one EditorView");
            // let spinner = editor_view.spinners_mut().get_or_create(server_id);
            // if spinner.is_stopped() {
            //   spinner.start();
            // }

            Ok(serde_json::Value::Null)
          },
          Ok(MethodCall::ApplyWorkspaceEdit(_params)) => {
            todo!("need to handle apply workspace edit");
          },
          Ok(MethodCall::WorkspaceFolders) => Ok(json!(&*language_server!().workspace_folders().await)),
          Ok(MethodCall::WorkspaceConfiguration(params)) => {
            let language_server = language_server!();
            let result: Vec<_> = params
              .items
              .iter()
              .map(|item| {
                let mut config = language_server.config()?;
                if let Some(section) = item.section.as_ref() {
                  // for some reason some lsps send an empty string (observed in 'vscode-eslint-language-server')
                  if !section.is_empty() {
                    for part in section.split('.') {
                      config = config.get(part)?;
                    }
                  }
                }
                Some(config)
              })
              .collect();
            Ok(json!(result))
          },
          Ok(MethodCall::RegisterCapability(params)) => {
            if let Some(client) = self.language_servers.iter_clients().find(|client| client.id() == server_id) {
              for reg in params.registrations {
                match reg.method.as_str() {
                  lsp::notification::DidChangeWatchedFiles::METHOD => {
                    let Some(options) = reg.register_options else {
                      continue;
                    };
                    let ops: lsp::DidChangeWatchedFilesRegistrationOptions = match serde_json::from_value(options) {
                      Ok(ops) => ops,
                      Err(err) => {
                        log::warn!("Failed to deserialize DidChangeWatchedFilesRegistrationOptions: {err}");
                        continue;
                      },
                    };
                    self.language_servers.file_event_handler.register(client.id(), Arc::downgrade(client), reg.id, ops)
                  },
                  _ => {
                    // Language Servers based on the `vscode-languageserver-node` library often send
                    // client/registerCapability even though we do not enable dynamic registration
                    // for most capabilities. We should send a MethodNotFound JSONRPC error in this
                    // case but that rejects the registration promise in the server which causes an
                    // exit. So we work around this by ignoring the request and sending back an OK
                    // response.
                    log::warn!("Ignoring a client/registerCapability request because dynamic capability registration is not enabled. Please report this upstream to the language server");
                  },
                }
              }
            }

            Ok(serde_json::Value::Null)
          },
          Ok(MethodCall::UnregisterCapability(params)) => {
            for unreg in params.unregisterations {
              match unreg.method.as_str() {
                lsp::notification::DidChangeWatchedFiles::METHOD => {
                  self.language_servers.file_event_handler.unregister(server_id, unreg.id);
                },
                _ => {
                  log::warn!("Received unregistration request for unsupported method: {}", unreg.method);
                },
              }
            }
            Ok(serde_json::Value::Null)
          },
          Ok(MethodCall::ShowDocument(params)) => {
            // let language_server = language_server!();
            // let offset_encoding = language_server.offset_encoding();

            // let result = self.handle_show_document(params, offset_encoding);:w
            todo!("need to handle show document");
            let result = serde_json::Value::Null;
            // Ok(json!(result))
          },
        };

        tokio::spawn(language_server!().reply(id, reply));
      },
      Call::Invalid { id } => log::error!("LSP invalid method call id={:?}", id),
    }
  }

  #[inline]
  pub fn clear_status(&mut self) {
    self.status_msg = None;
  }

  #[inline]
  pub fn set_status<T: Into<Cow<'static, str>>>(&mut self, status: T) {
    let status = status.into();
    log::debug!("editor status: {}", status);
    self.status_msg = Some((status, Severity::Info));
  }

  #[inline]
  pub fn set_error<T: Into<Cow<'static, str>>>(&mut self, error: T) {
    let error = error.into();
    log::debug!("editor error: {}", error);
    self.status_msg = Some((error, Severity::Error));
  }

  #[inline]
  pub fn get_status(&self) -> Option<(&Cow<'static, str>, &Severity)> {
    self.status_msg.as_ref().map(|(status, sev)| (status, sev))
  }
}
