use arc_swap::ArcSwap;
use futures_util::FutureExt;
use helix_core::diff::compare_ropes;
use helix_core::syntax;
use std::borrow::Cow;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use helix_core::diagnostic::Severity;
use helix_core::syntax::LanguageConfiguration;
use helix_core::syntax::Loader;
use helix_lsp::lsp::{self, notification::Notification};
use helix_lsp::Call;
use helix_lsp::Client;
use helix_lsp::LspProgressMap;
use helix_lsp::Registry;
use log::{error, info};
use lsp::DocumentSymbol;
use lsp::NumberOrString;
use serde_json::json;

use url::Url;

use crate::action::SessionAction;
use crate::app::lsp::symbol_types::DocumentChange;
use crate::app::lsp::workspace::Workspace;
use crate::app::model_tools::lsp_tool::LsiAction;
use crate::trace_dbg;

use super::symbol_types::SourceSymbol;
use super::symbol_types::SymbolQuery;

#[derive(Debug, Default)]
pub struct StatusMessage {
  pub msg: Option<(Cow<'static, str>, Severity)>,
}

impl StatusMessage {
  #[inline]
  pub fn clear_status(&mut self) {
    self.msg = None;
  }

  #[inline]
  pub fn set_status<T: Into<Cow<'static, str>>>(&mut self, status: T) {
    let status = status.into();
    log::debug!("editor status: {}", status);
    self.msg = Some((status, Severity::Info));
  }

  #[inline]
  pub fn set_error<T: Into<Cow<'static, str>>>(&mut self, error: T) {
    let error = error.into();
    log::debug!("editor error: {}", error);
    self.msg = Some((error, Severity::Error));
  }

  #[inline]
  pub fn get_status(&self) -> Option<(&Cow<'static, str>, &Severity)> {
    if let Some((status, severity)) = &self.msg {
      Some((status, severity))
    } else {
      None
    }
  }
}

#[derive(Debug)]
pub struct LanguageServerInterface {
  pub workspaces: Vec<Workspace>,
  pub lsp_progress: LspProgressMap,
  pub language_servers: helix_lsp::Registry,
  pub status_msg: StatusMessage,
  loader: Arc<ArcSwap<Loader>>,
  pub tx: UnboundedSender<LsiAction>,
}

impl LanguageServerInterface {
  pub fn new(
    syn_loader: Arc<ArcSwap<syntax::Loader>>,
    tx: UnboundedSender<LsiAction>,
  ) -> Self {
    let loader = syn_loader.clone();
    // let language_servers = Arc::new(Mutex::new(Registry::new(loader.clone())))
    let language_servers = helix_lsp::Registry::new(syn_loader.clone());
    Self {
      lsp_progress: LspProgressMap::new(),
      loader,
      language_servers,
      status_msg: StatusMessage::default(),
      workspaces: vec![],
      tx,
    }
  }

  pub async fn handle_action(
    &mut self,
    action: LsiAction,
  ) -> Result<Option<LsiAction>, anyhow::Error> {
    match action {
      LsiAction::Error(error) => {
        log::error!("{}", error);
        Ok(None)
      },
      LsiAction::AddWorkspace(ws) => {
        if let Err(e) = self.create_workspace(
          ws.workspace_path,
          &ws.language,
          &ws.language_server,
          ws.doc_path.as_ref(),
        ) {
          self
            .tx
            .send(LsiAction::Error(format!("error creating workspace: {}", e)))
            .unwrap();
        };
        self.update_workspace_symbols().await?;
        Ok(None)
      },
      LsiAction::QueryWorkspaceSymbols(
        query,
        workspace_root,
        session_id,
        tool_call_id,
      ) => {
        log::info!("query_workspace_symbols: {:#?}", query);
        self.update_workspace_symbols().await?;
        match self
          .workspaces
          .iter()
          .find(|w| w.workspace_path == workspace_root)
        {
          Some(workspace) => {
            self
              .query_workspace_symbols(
                query,
                workspace,
                session_id,
                tool_call_id,
              )
              .await;
            Ok(None)
          },
          None => {
            self
              .tx
              .send(LsiAction::Error(format!(
                "no workspace found at {}",
                workspace_root.display()
              )))
              .unwrap();
            Ok(None)
          },
        }
      },
      _ => Ok(None),
    }
  }

  pub async fn query_workspace_symbols(
    &self,
    query: SymbolQuery,
    workspace: &Workspace,
    session_id: i64,
    tool_call_id: String,
  ) {
    match workspace.query_symbols(&query).await {
      Ok(symbols) => {
        log::info!("symbols: {:#?}", symbols.len());
        let content = match symbols.len() {
          0 => "no symbols found".to_string(),
          _ => match serde_json::to_string(&symbols) {
            Ok(content) => content,
            Err(e) => {
              log::error!("error serializing symbols: {}", e);
              "".to_string()
            },
          },
        };
        log::info!("symbols size: {:#?}", content.len());
        self
          .tx
          .send(LsiAction::SessionAction(Box::new(
            SessionAction::ToolCallComplete(session_id, tool_call_id, content),
          )))
          .unwrap();
      },
      Err(e) => {
        let error = format!("error querying workspace symbols: {}", e);
        log::error!("{}", error);
        self
          .tx
          .send(LsiAction::Error(error))
          .expect("failed to send error message");
      },
    };
  }

  pub async fn query_all_workspace_symbols(
    &self,
    query: SymbolQuery,
    //   name: Option<String>,
    //   kind: Option<lsp::SymbolKind>,
    //   range: Option<lsp::Range>,
    //   file: Option<String>,
  ) -> Vec<Rc<SourceSymbol>> {
    //   let query =
    //     SymbolQuery { name: name.clone(), kind, range, file: file.clone() };
    futures::future::join_all(
      self
        .workspaces
        .iter()
        .map(|w| async { w.query_symbols(&query).await })
        .collect::<Vec<_>>(),
    )
    .await
    .iter()
    .flatten()
    .flatten()
    .cloned()
    .collect()
  }

  // pub async fn spawn_server_notification_thread(&mut self) {
  //   log::info!("spawn_server_notification_thread");
  //   use futures_util::StreamExt;
  //   let ls_mutex = self.language_servers.clone();
  //   let action_tx = self.action_tx.clone().unwrap();
  //
  //   let mut interval =
  //     tokio::time::interval(std::time::Duration::from_millis(500));
  //   tokio::spawn(async move {
  //     loop {
  //       let mut ls = ls_mutex.lock().await;
  //
  //       if let Some((id, call)) = ls.incoming.next().await {
  //         action_tx.send(Action::LspServerMessageReceived((id, call))).unwrap();
  //       }
  //       interval.tick().await;
  //     }
  //   });
  // }

  // pub async fn check_server_notifications(
  //   &mut self,
  // ) -> Next<'_, SelectAll<UnboundedReceiverStream<(usize, Call)>>> {
  //   use futures_util::StreamExt;
  //   trace_dbg!("check_server_notifications");
  //
  //   let mut ls = self.language_servers.lock().await;
  //
  //   ls.incoming.next()
  // }

  pub async fn server_capabilities(
    &self,
  ) -> anyhow::Result<Vec<lsp::ServerCapabilities>> {
    // let ls = self.language_servers.lock().await;
    Ok(
      self
        .language_servers
        .iter_clients()
        .map(|client| client.capabilities().clone())
        .collect::<Vec<_>>(),
    )
  }

  pub async fn goto_symbol_definition(
    &self,
    symbol: Rc<SourceSymbol>,
    language_server_id: usize,
  ) -> anyhow::Result<lsp::GotoDefinitionResponse> {
    let text_document = lsp::TextDocumentIdentifier {
      uri: Url::from_file_path(symbol.file_path.clone()).unwrap(),
    };
    let position = symbol.selection_range.borrow().start;

    let client = self
      .language_servers
      .iter_clients()
      .find(|c| c.id() == language_server_id)
      .expect("could not obtain language server for goto request")
      .clone();

    let work_done_token =
      Some(NumberOrString::String("goto definition".to_string()));
    let request = client
      .goto_definition(text_document, position, work_done_token)
      .expect("could not obtain goto definition response");
    Ok(serde_json::from_value(request.await?)?)
  }

  pub async fn goto_symbol_declaration(
    &self,
    symbol: Rc<SourceSymbol>,
    language_server_id: usize,
  ) -> anyhow::Result<lsp::GotoDefinitionResponse> {
    let text_document = lsp::TextDocumentIdentifier {
      uri: Url::from_file_path(symbol.file_path.clone()).unwrap(),
    };
    let position = symbol.selection_range.borrow().start;
    let client = self
      .language_servers
      .iter_clients()
      .find(|c| c.id() == language_server_id)
      .expect("could not obtain language server for goto request")
      .clone();
    let work_done_token =
      Some(NumberOrString::String("goto declaration".to_string()));
    let request = client
      .goto_declaration(text_document, position, work_done_token)
      .expect("could not obtain goto declaration response");
    Ok(serde_json::from_value(request.await?)?)
  }

  pub async fn goto_type_definition(
    &self,
    symbol: Arc<SourceSymbol>,
    language_server_id: usize,
  ) -> anyhow::Result<lsp::Location> {
    let text_document = lsp::TextDocumentIdentifier {
      uri: Url::from_file_path(symbol.file_path.clone()).unwrap(),
    };
    let position = symbol.selection_range.borrow().start;
    let client = self
      .language_servers
      .iter_clients()
      .find(|c| c.id() == language_server_id)
      .expect("could not obtain language server for goto request")
      .clone();
    let work_done_token =
      Some(NumberOrString::String("goto type definition".to_string()));
    let request = client
      .goto_type_definition(text_document, position, work_done_token)
      .expect("could not obtain goto type definition response");
    Ok(serde_json::from_value(request.await?)?)
  }

  pub fn create_workspace(
    &mut self,
    workspace_path: PathBuf,
    language_name: &str,
    languge_server_name: &str,
    doc_path: Option<&PathBuf>,
  ) -> anyhow::Result<()> {
    log::info!("create_workspace: {:#?}", workspace_path);

    let root_dirs = &[workspace_path.clone()];
    let enable_snippets = false;

    let language_server = self
      .initialize_client(
        language_name,
        languge_server_name,
        doc_path,
        root_dirs,
        enable_snippets,
      )
      .unwrap()
      .expect("unable to initialize language server");

    let language_server_id = language_server.id();
    let language_config = self
      .language_configuration_by_name(language_name)
      .expect("can't find language configuration");
    self.workspaces.push(Workspace::new(
      &workspace_path,
      language_name.into(),
      language_server_id,
      language_config,
      helix_lsp::OffsetEncoding::default(),
    ));
    Ok(())
  }

  pub async fn update_workspace_symbols(&mut self) -> anyhow::Result<()> {
    log::info!("update_workspace_symbols");

    let clients = self
      .language_servers
      .iter_clients()
      .cloned()
      .collect::<Vec<Arc<Client>>>();
    let ids = clients.iter().map(|client| client.id()).collect::<Vec<usize>>();

    self.wait_for_language_server_initialization(&ids).await?;
    match self.wait_for_progress_token_completion(ids.as_slice()).await {
      Ok(_) => {
        log::info!("update_workspace_symbols: {:#?}", ids);
        for workspace in self.workspaces.iter_mut() {
          workspace.scan_workspace_files().unwrap();
          log::info!("workspace files: {:#?}", workspace.files.len());
          for workspace_file in
            workspace.files.iter_mut().filter(|workspace_file| {
              workspace_file.needs_update().unwrap_or_default()
            })
          {
            let language_server = clients
              .iter()
              .find(|client| client.id() == workspace.language_server_id)
              .expect("cannot find workspace language server");

            workspace.offset_encoding = language_server.offset_encoding();
            log::info!(
              "updating workspace file: {:#?}",
              workspace_file.file_path
            );

            // update workspace file contents
            let doc_change = workspace_file.update_contents().unwrap();
            log::info!("document change: {:#?}", doc_change);
            if let DocumentChange {
              original_contents: Some(original_contents),
              new_contents,
              versioned_doc_id,
            } = doc_change
            {
              log::info!(
                "updating document with language server {}",
                workspace_file.file_path.display()
              );

              let changes = compare_ropes(&original_contents, &new_contents);
              language_server
                .text_document_did_change(
                  versioned_doc_id,
                  &original_contents,
                  &new_contents,
                  changes.changes(),
                )
                .unwrap()
                .then(|res| async move {
                  log::info!("updated document with language server");
                  res
                })
                .await
                .expect("failed to update document with language server")
            } else {
              log::info!(
                "opening document with language server {}",
                workspace_file.file_path.display()
              );
              language_server
                .text_document_did_open(
                  doc_change.versioned_doc_id.uri,
                  workspace_file.version,
                  &doc_change.new_contents,
                  workspace.language_id.clone(),
                )
                .then(|res| async move {
                  log::info!("opened document with language server");
                  res
                })
                .await
                .expect("failed to open document with language server")
            }

            if let Some(request) = language_server
              .document_symbols(workspace_file.get_text_document_id().unwrap())
            {
              log::info!(
                "requesting document symbols for {}",
                workspace_file.file_path.display()
              );
              let response_json = request.await.unwrap();
              let response_parsed: Option<lsp::DocumentSymbolResponse> =
                serde_json::from_value(response_json)?;

              let doc_symbols = match response_parsed {
                Some(lsp::DocumentSymbolResponse::Nested(symbols)) => {
                  log::info!("nested symbols: {:#?}", symbols);
                  symbols
                  // let mut flat_symbols = Vec::new();
                  // for symbol in symbols {
                  //   nested_to_flat(&mut flat_symbols, &doc_id, symbol, offset_encoding)
                  // }
                  // flat_symbols
                },
                Some(lsp::DocumentSymbolResponse::Flat(_symbols)) => {
                  log::info!("flat symbols: {:#?}", _symbols);
                  // symbols.into_iter().map(|symbol| SymbolInformationItem { symbol, offset_encoding }).collect()
                  return Err(anyhow::anyhow!(
                    "document symbol support is required"
                  ));
                },
                None => {
                  log::info!("document symbol response is None");
                  return Err(anyhow::anyhow!(
                    "document symbol response is None"
                  ));
                },
              };
              workspace_file.update_symbols(doc_symbols).unwrap();
              log::debug!(
                "workspace_file symbols: {:#?}",
                workspace_file.file_tree
              );
            }
          }
        }
        Ok(())
      },
      Err(e) => Err(e),
    }
  }

  pub async fn query_document_symbols(
    &mut self,
    doc_url: &Url,
    ids: &[usize],
  ) -> anyhow::Result<Vec<DocumentSymbol>> {
    match self.wait_for_progress_token_completion(ids).await {
      Ok(_) => {
        let mut results = vec![];
        for language_server in self.language_servers.iter_clients() {
          if ids.contains(&language_server.id()) {
            let doc_id = lsp::TextDocumentIdentifier::new(doc_url.clone());

            let _offset_encoding = language_server.offset_encoding();
            if let Some(s) = language_server.document_symbols(doc_id.clone()) {
              let symbols = s.await.unwrap();
              let response: Option<lsp::DocumentSymbolResponse> =
                serde_json::from_value(symbols)?;

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
                  return Err(anyhow::anyhow!(
                    "document symbol support is required"
                  ));
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

  pub async fn wait_for_language_server_initialization(
    &mut self,
    language_server_ids: &[usize],
  ) -> anyhow::Result<()> {
    log::info!(
      "wait_for_language_server_initialization: {:#?}",
      language_server_ids
    );

    let active_clients = self
      .language_servers
      .iter_clients()
      .filter(|client| language_server_ids.contains(&client.id()))
      .cloned()
      .collect::<Vec<Arc<Client>>>();

    tokio::spawn({
      async move {
        let mut interval =
          tokio::time::interval(std::time::Duration::from_secs(1));
        while !active_clients.iter().all(|client| client.is_initialized()) {
          interval.tick().await;
          log::info!("waiting for language server initialization");
        }
        trace_dbg!("all clients initialized");
      }
    })
    .await
    .map_err(|e| anyhow::anyhow!(e))
  }

  pub async fn wait_for_progress_token_completion(
    &self,
    ids: &[usize],
  ) -> anyhow::Result<()> {
    log::info!("wait_for_progress_token_completion: {:#?}", ids);
    // let ls = self.language_servers.lock().await;
    //
    // let active_clients = ls
    //   .iter_clients()
    //   .filter(|client| ids.contains(&client.id()))
    //   .cloned()
    //   .collect::<Vec<Arc<Client>>>();
    //
    // // log::info!("active_clients: {:#?}", active_clients);
    //
    // if active_clients.is_empty() {
    //   trace_dbg!("no language servers with matching ids found: {:#?}", ids);
    //   return Err(anyhow::anyhow!(
    //     "no language servers with matching ids found"
    //   ));
    // }
    //
    // self
    //   .wait_for_language_server_initialization(ids)
    //   .then(|_| async { log::info!("language server initialized") })
    //   .await;

    log::info!("waiting for progress token completion loop");
    let mut interval =
      tokio::time::interval(std::time::Duration::from_millis(1000));
    while ids.iter().any(|c| {
      log::info!("lsp_progress: {:#?}", self.lsp_progress.progress_map(*c));
      self.lsp_progress.is_progressing(*c)
    }) {
      interval.tick().await;
    }
    Ok(())
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
        let client = self
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
          .map_err(|e| anyhow::anyhow!(e))?;
        Ok(Some(client))
      },
      None => Ok(None),
    }
  }

  pub fn language_configuration_by_name(
    &self,
    name: &str,
  ) -> Option<Arc<LanguageConfiguration>> {
    self.loader.load().language_config_for_name(name)
  }

  pub async fn language_server_by_name(
    &self,
    language_server_name: String,
  ) -> Option<Arc<helix_lsp::Client>> {
    let client = self
      .language_servers
      .iter_clients()
      .find(|client| client.name() == language_server_name);
    client.cloned()
  }

  pub async fn language_server_by_id(
    &self,
    language_server_id: usize,
  ) -> Option<Arc<helix_lsp::Client>> {
    let client = self
      .language_servers
      .iter_clients()
      .find(|client| client.id() == language_server_id);
    client.cloned()
  }

  pub async fn handle_language_server_message(
    lsp_progress: &mut LspProgressMap,
    registry: &mut Registry,
    call: helix_lsp::Call,
    server_id: usize,
    status_msg: &mut StatusMessage,
    workspaces: &mut [Workspace],
  ) {
    log::debug!("handle_language_server_message: {:#?}", call);
    use helix_lsp::{MethodCall, Notification};

    macro_rules! language_server {
      () => {
        registry
          .iter_clients()
          .find(|client| client.id() == server_id)
          .expect("expected language server")
        // match self.language_server_by_id(server_id).await {
        //   Some(language_server) => language_server,
        //   None => {
        //     warn!("can't find language server with id `{}`", server_id);
        //     return;
        //   },
        // }
      };
    }

    match call {
      Call::Notification(helix_lsp::jsonrpc::Notification {
        method,
        params,
        ..
      }) => {
        let notification = match Notification::parse(&method, params) {
          Ok(notification) => notification,
          Err(helix_lsp::Error::Unhandled) => {
            info!("Ignoring Unhandled notification from Language Server");
            return;
          },
          Err(err) => {
            error!(
              "Ignoring unknown notification from Language Server: {}",
              err
            );
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
              tokio::spawn(
                language_server.did_change_configuration(config.clone()),
              );
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
            log::info!("Language server initialized: server id: {}", server_id);
          },
          Notification::PublishDiagnostics(params) => {
            let file_path = params.uri.to_file_path().unwrap();
            match workspaces
              .iter_mut()
              .find_map(|ws| ws.get_mut_file(&file_path))
            {
              Some(file) => {
                let new_diagnostics = params.diagnostics;
                match params.version {
                  Some(version) => {
                    if let Some(diagnostics) =
                      file.diagnostics.get_mut(&version)
                    {
                      diagnostics.extend(new_diagnostics);
                      log::info!(
                        "updated diagnostics for version: {}",
                        version
                      );
                      log::debug!("diagnostics: {:#?}", diagnostics);
                    } else {
                      file.diagnostics.insert(version, new_diagnostics.clone());
                      log::info!("added diagnostics for version: {}", version);
                      log::debug!("diagnostics: {:#?}", new_diagnostics);
                    }
                  },
                  None => {
                    log::warn!("no version supplied with server message, using file version {}", file.version);
                    if let Some(diagnostics) =
                      file.diagnostics.get_mut(&file.version)
                    {
                      diagnostics.extend(new_diagnostics);
                      log::info!(
                        "updated diagnostics for version: {}",
                        file.version
                      );
                      log::debug!("diagnostics: {:#?}", diagnostics);
                    } else {
                      file
                        .diagnostics
                        .insert(file.version, new_diagnostics.clone());
                      log::info!(
                        "added diagnostics for file.version: {}",
                        file.version
                      );
                      log::debug!("diagnostics: {:#?}", new_diagnostics);
                    }
                  },
                };
              },
              None => {
                log::error!("no workspace file found for uri: {:?}", file_path);
              },
            }
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
            let parts =
              match &work {
                lsp::WorkDoneProgress::Begin(lsp::WorkDoneProgressBegin {
                  title,
                  message,
                  percentage,
                  ..
                }) => (Some(title), message, percentage),
                lsp::WorkDoneProgress::Report(
                  lsp::WorkDoneProgressReport { message, percentage, .. },
                ) => (None, message, percentage),
                lsp::WorkDoneProgress::End(lsp::WorkDoneProgressEnd {
                  message,
                }) => {
                  log::error!("UNKNOWN MESSAGE: {:#?}", message);
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
              (None, None, None) => {
                format!("[{}]", token_d)
              },
            };

            if let lsp::WorkDoneProgress::End(a) = work {
              let res = lsp_progress.end_progress(server_id, &token);
              log::info!("end progress: {:#?} {:#?}", res, a);
              // if !self.lsp_progress.is_progressing(server_id) {
              // editor_view.spinners_mut().get_or_create(server_id).stop();
              // }
            } else {
              lsp_progress.update(server_id, token, work);
            }
            //
            // self.lsp_progress.update(server_id, token, work);
            // if self.config.load().editor.lsp.display_messages {
            log::debug!("status: {}", status);
            status_msg.set_status(status);
            // }
          },
          Notification::Exit => {
            status_msg.set_status("Language server exited");

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
            registry.remove_by_id(server_id);
          },
        }
      },
      Call::MethodCall(helix_lsp::jsonrpc::MethodCall {
        method,
        params,
        id,
        ..
      }) => {
        let reply = match MethodCall::parse(&method, params) {
          Err(helix_lsp::Error::Unhandled) => {
            error!(
              "Language Server: Method {} not found in request {}",
              method, id
            );
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
          Ok(MethodCall::WorkspaceFolders) => {
            Ok(json!(&*language_server!().workspace_folders().await))
          },
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
            if let Some(client) =
              registry.iter_clients().find(|client| client.id() == server_id)
            {
              for reg in params.registrations {
                match reg.method.as_str() {
                  lsp::notification::DidChangeWatchedFiles::METHOD => {
                    let Some(options) = reg.register_options else {
                      continue;
                    };
                    let ops: lsp::DidChangeWatchedFilesRegistrationOptions =
                      match serde_json::from_value(options) {
                        Ok(ops) => ops,
                        Err(err) => {
                          log::warn!("Failed to deserialize DidChangeWatchedFilesRegistrationOptions: {err}");
                          continue;
                        },
                      };
                    registry.file_event_handler.register(
                      client.id(),
                      Arc::downgrade(client),
                      reg.id,
                      ops,
                    )
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
                  registry.file_event_handler.unregister(server_id, unreg.id);
                },
                _ => {
                  log::warn!("Received unregistration request for unsupported method: {}", unreg.method);
                },
              }
            }
            Ok(serde_json::Value::Null)
          },
          Ok(MethodCall::ShowDocument(_params)) => {
            // let language_server = language_server!();
            // let offset_encoding = language_server.offset_encoding();

            // let result = self.handle_show_document(params, offset_encoding);:w
            log::error!("need to handle show document");
            todo!("need to handle show document");
            // let _result = serde_json::Value::Null;
            // Ok(json!(result))
          },
        };

        tokio::spawn(language_server!().reply(id, reply));
      },
      Call::Invalid { id } => {
        log::error!("LSP invalid method call id={:?}", id)
      },
    }
    // pub async fn get_semantic_tokens(&mut self, doc_url: &Url, id: usize) -> anyhow::Result<lsp::SemanticTokensResult> {
    //   let language_server = self.language_server_by_id(id).unwrap();
    //   let doc_id = lsp::TextDocumentIdentifier::new(doc_url.clone());
    //   if let Some(s) = language_server.semantic_tokens(doc_id.clone()) {
    //     let tokens = s.await.unwrap();
    //     let response: Option<lsp::SemanticTokensResult> = serde_json::from_value(tokens)?;
    //     let tokens = match response {
    //       Some(tokens) => tokens,
    //       None => return Err(anyhow::anyhow!("no semantic tokens found")),
    //     };
    //     Ok(tokens)
    //   } else {
    //     Err(anyhow::anyhow!("no semantic tokens found"))
    //   }
    // }

    // pub async fn query_workspace_symbols(
    //   &mut self,
    //   query: &str,
    //   ids: &[usize],
    // ) -> anyhow::Result<Vec<lsp::WorkspaceSymbol>> {
    //   match self.wait_for_progress_token_completion(ids).await {
    //     Ok(_) => {
    //       let mut results = vec![];
    //       for client in self.language_servers.iter_clients() {
    //         println!("client id: {}", client.id());
    //
    //         if ids.contains(&client.id()) {
    //           println!("client name is included: {}", client.name());
    //
    //           if let Some(s) = client.workspace_symbols(query.into()) {
    //             let symbols = s.await.unwrap();
    //             results
    //               .extend(from_value::<Vec<lsp::WorkspaceSymbol>>(symbols).expect("failed to parse workspace symbols"))
    //           }
    //         }
    //       }
    //       Ok(results)
    //     },
    //     Err(e) => Err(e),
    //   }
    // }

    // async fn get_workspace_files(&mut self, id: usize) -> anyhow::Result<Vec<PathBuf>> {
    //   let mut files: Vec<PathBuf> = Vec::new();
    //
    //   match self.language_server_by_id(id) {
    //     Some(language_server) => {
    //       let workspace_folders = language_server.workspace_folders();
    //       let wf = workspace_folders.await;
    //       for folder in wf.iter() {
    //         let folderfiles = walkdir::WalkDir::new(folder.uri.to_file_path().unwrap())
    //           .into_iter()
    //           .filter_map(|e| e.ok())
    //           .filter(|e| e.path().is_file())
    //           .filter(|e| e.path().extension().unwrap_or_default() == "rs")
    //           .flat_map(|e| e.path().canonicalize())
    //           .collect::<Vec<PathBuf>>();
    //         files.extend(folderfiles);
    //       }
    //     },
    //     None => return Err(anyhow::anyhow!("no language server with id found")),
    //   }
    //   println!("files: {:#?}", files);
    //   Ok(files)
    // }
    //
    // pub async fn get_workspace_document_symbols(&mut self, id: usize) -> anyhow::Result<Vec<DocumentSymbol>> {
    //   log::debug!("get_workspace_document_symbols: {:#?}", id);
    //   let files = self.get_workspace_files(id).await?;
    //   let mut doc_symbols = vec![];
    //   for file in files.iter() {
    //     let uri = Url::from_file_path(file).unwrap();
    //     log::debug!("uri: {:#?}", uri);
    //     let symbols = self.query_document_symbols(&uri, &[id]).await.unwrap();
    //     doc_symbols.extend(symbols);
    //   }
    //   Ok(doc_symbols)
    // }
  }
}
