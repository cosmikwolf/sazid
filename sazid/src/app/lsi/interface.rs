use arc_swap::ArcSwap;
use futures_util::FutureExt;
use helix_core::diff::compare_ropes;
use helix_core::syntax;
use helix_lsp::Registry;
use lsp::TextDocumentIdentifier;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

use helix_core::syntax::LanguageConfiguration;
use helix_core::syntax::Loader;
use helix_lsp::lsp;
use helix_lsp::Client;
use helix_lsp::LspProgressMap;
use lsp::DocumentSymbol;

use url::Url;

use crate::action::LsiAction;
use crate::action::SessionAction;
use crate::action::ToolType;
use crate::app::lsi::symbol_types::DocumentChange;
use crate::app::lsi::workspace::Workspace;

use super::query::LsiQuery;

#[derive(Debug)]
pub struct LanguageServerInterface {
  pub workspaces: Vec<Workspace>,
  pub lsp_progress: LspProgressMap,
  pub language_servers: Registry,
  loader: Arc<ArcSwap<Loader>>,
  pub tx: UnboundedSender<LsiAction>,
}

impl LanguageServerInterface {
  pub fn new(syn_loader: Arc<ArcSwap<syntax::Loader>>, tx: UnboundedSender<LsiAction>) -> Self {
    let loader = syn_loader.clone();
    // let language_servers = Arc::new(Mutex::new(Registry::new(loader.clone())))
    let language_servers = Registry::new(syn_loader.clone());
    Self { lsp_progress: LspProgressMap::new(), loader, language_servers, workspaces: vec![], tx }
  }

  pub fn handle_action(&mut self, action: LsiAction) {
    let action_result = match action {
      LsiAction::Error(error) => {
        log::error!("{}", error);
        Ok(None)
      },
      LsiAction::GetWorkspaceFiles(lsi_query) => {
        log::info!("get_workspace_files: {:#?}", lsi_query);
        let lsi_query_result = self.get_workspace_files(&lsi_query);
        Self::handle_lsi_query_result(lsi_query, lsi_query_result)
      },
      LsiAction::AddWorkspace(ws) => {
        match self.create_workspace(
          ws.workspace_path,
          &ws.language,
          &ws.language_server,
          ws.doc_path.as_ref(),
        ) {
          Ok(()) => match self.synchronize_workspace_file_changes() {
            Ok(()) => Ok(None),
            Err(e) => {
              Ok(Some(LsiAction::Error(format!("error updating workspace symbols: {}", e))))
            },
          },
          Err(e) => Ok(Some(LsiAction::Error(format!("error creating workspace: {}", e)))),
        }
      },
      LsiAction::QueryWorkspaceSymbols(lsi_query) => {
        log::info!("query_workspace_symbols: {:#?}", lsi_query);
        let lsi_query_result = self.lsi_query_workspace_symbols(&lsi_query);
        Self::handle_lsi_query_result(lsi_query, lsi_query_result)
      },
      LsiAction::SessionAction(_) => Ok(None),
      LsiAction::ChatToolResponse(_) => Ok(None),
      LsiAction::GoToSymbolDefinition(lsi_query) => {
        log::info!("goto_symbol_definition: {:#?}", lsi_query);
        self.goto_symbol_definition(&lsi_query).expect("goto_symbol_definition failed");
        Ok(None)
      },
      LsiAction::GoToSymbolDeclaration(lsi_query) => {
        log::info!("goto_symbol_declaration: {:#?}", lsi_query);
        self.goto_symbol_declaration(&lsi_query).unwrap();
        Ok(None)
        // self.handle_lsi_query_response(lsi_query, lsi_query_result)
      },
      LsiAction::GoToTypeDefinition(lsi_query) => {
        log::info!("goto_type_definition: {:#?}", lsi_query);
        self.goto_type_definition(&lsi_query).expect("goto_type_definition failed");
        Ok(None)
      },
      LsiAction::GetDiagnostics(lsi_query) => {
        log::info!("get_diagnostics: {:#?}", lsi_query);
        let lsi_query_result = self.get_diagnostics(&lsi_query);
        Self::handle_lsi_query_result(lsi_query, lsi_query_result)
      },
      LsiAction::SynchronizeAllWorkspaceFileChanges() => {
        match self.synchronize_workspace_file_changes() {
          Ok(()) => Ok(None),
          Err(e) => Ok(Some(LsiAction::Error(format!("error updating workspace symbols: {}", e)))),
        }
      },
      LsiAction::UpdateWorkspaceFileSymbols(workspace_path, doc_id, doc_symbols) => {
        // log::info!(
        //   "update {} workspace file symbols for doc id: {:#?}, ",
        //   doc_symbols.len(),
        //   doc_id.uri.path()
        // );
        match self
          .workspaces
          .iter_mut()
          .find(|workspace| workspace.workspace_path == workspace_path)
        {
          Some(workspace) => {
            workspace.replace_doc_symbols(doc_id, doc_symbols).expect("replace_doc_symbols failed");
            Ok(None)
          },
          None => Ok(Some(LsiAction::Error(format!(
            "cannot update workspace symbols, workspace not found at {:?}",
            workspace_path
          )))),
        }
      },
      LsiAction::RequestWorkspaceFileSymbols(workspace_path, doc_id, language_server_id) => {
        // log::info!("get workspace file symbols: {:#?}", doc_id);
        let language_server = self.language_server_by_id(language_server_id).unwrap();
        let tx = self.tx.clone();
        match Self::get_workspace_file_symbols(workspace_path, doc_id, language_server, tx) {
          Ok(()) => Ok(None),
          Err(e) => {
            Ok(Some(LsiAction::Error(format!("error getting workspace file symbols: {}", e))))
          },
        }
      },
    };

    match action_result {
      Ok(Some(action)) => {
        self.tx.send(action).unwrap();
      },
      Ok(None) => (),
      Err(e) => {
        log::error!("error lsi handling action: {:#?}", e);
        self.tx.send(LsiAction::Error(e.to_string())).unwrap();
      },
    }
  }

  pub fn send_query_response(
    tx: &UnboundedSender<LsiAction>,
    lsi_query: LsiQuery,
    result: anyhow::Result<String>,
  ) {
    match Self::handle_lsi_query_result(lsi_query, result) {
      Ok(Some(action)) => tx.send(action).unwrap(),
      Ok(None) => (),
      Err(e) => {
        log::error!("error lsi handling action: {:#?}", e);
        tx.send(LsiAction::Error(e.to_string())).unwrap();
      },
    }
  }

  pub fn handle_lsi_query_result(
    lsi_query: LsiQuery,
    result: anyhow::Result<String>,
  ) -> anyhow::Result<Option<LsiAction>> {
    log::info!("lsi_query_result: {:#?}", result);
    match result {
      Ok(response) => Ok(Some(LsiAction::SessionAction(Box::new(
        SessionAction::ToolCallComplete(ToolType::LsiQuery(lsi_query), response),
      )))),
      Err(e) => Ok(Some(LsiAction::SessionAction(Box::new(SessionAction::ToolCallError(
        ToolType::LsiQuery(lsi_query),
        e.to_string(),
      ))))),
    }
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

  pub async fn server_capabilities(&self) -> anyhow::Result<Vec<lsp::ServerCapabilities>> {
    // let ls = self.language_servers.lock().await;
    Ok(
      self
        .language_servers
        .iter_clients()
        .map(|client| client.capabilities().clone())
        .collect::<Vec<_>>(),
    )
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
      .initialize_client(language_name, languge_server_name, doc_path, root_dirs, enable_snippets)
      .unwrap()
      .expect("unable to initialize language server");

    tokio::time::interval(Duration::from_millis(250));
    while !language_server.is_initialized() {
      // log::info!("waiting for language server to initialize");
    }

    let language_config = self
      .language_configuration_by_name(language_name)
      .expect("can't find language configuration");
    self.workspaces.push(Workspace::new(
      &workspace_path,
      language_name.into(),
      language_server,
      language_config,
    ));
    Ok(())
  }

  pub fn scan_for_workspace_file_changes(
    &mut self,
  ) -> Vec<(PathBuf, DocumentChange, TextDocumentIdentifier, i32, Arc<Client>, String)> {
    self
      .workspaces
      .iter_mut()
      .flat_map(|workspace| {
        workspace.scan_workspace_files().unwrap();
        let language_server = workspace.language_server.clone();
        let language_id = workspace.language_id.clone();
        log::info!("workspace files: {:#?}", workspace.files.len());
        workspace
          .files
          .iter_mut()
          .filter(|workspace_file| workspace_file.needs_update().unwrap_or_default())
          .map(move |workspace_file| {
            // log::info!("updating workspace file: {:#?}", workspace_file.file_path);

            (
              workspace_file.workspace_path.clone(),
              workspace_file.update_contents().unwrap(),
              workspace_file.get_text_document_id().unwrap(),
              workspace_file.version,
              language_server.clone(),
              language_id.clone(),
            )
          })
      })
      .collect()
  }

  pub fn synchronize_workspace_file_changes(&mut self) -> anyhow::Result<()> {
    self.workspaces.iter_mut().for_each(|workspace| workspace.scan_workspace_files().unwrap());
    let changes = self.scan_for_workspace_file_changes();
    for (workspace_path, doc_change, doc_id, version, language_server, language_id) in changes {
      if let DocumentChange {
        original_contents: Some(original_contents),
        new_contents,
        versioned_doc_id,
      } = doc_change
      {
        log::info!("updating document with language server {:#?}", doc_id);
        let changes = compare_ropes(&original_contents, &new_contents);
        let tx = self.tx.clone();
        tokio::spawn(async move {
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
              match res {
                Err(e) => {
                  log::error!("failed to update document with language server: {}", e);
                },
                Ok(()) => {
                  tx.send(LsiAction::RequestWorkspaceFileSymbols(
                    workspace_path,
                    doc_id,
                    language_server.id(),
                  ))
                  .unwrap();
                },
              }
            })
            .await
        });
      } else {
        let tx = self.tx.clone();
        tokio::spawn(async move {
          language_server
            .text_document_did_open(
              doc_change.versioned_doc_id.uri,
              version,
              &doc_change.new_contents,
              language_id,
            )
            .then(|res| async move {
              // log::info!("updated document with language server");
              match res {
                Err(e) => {
                  log::error!("failed to open document with language server: {}", e);
                },
                Ok(()) => {
                  tx.send(LsiAction::RequestWorkspaceFileSymbols(
                    workspace_path,
                    doc_id,
                    language_server.id(),
                  ))
                  .unwrap();
                },
              }
            })
            .await
        });
      }
    }
    Ok(())
  }

  pub fn get_workspace_file_symbols(
    workspace_path: PathBuf,
    doc_id: TextDocumentIdentifier,
    language_server: Arc<Client>,
    tx: UnboundedSender<LsiAction>,
  ) -> anyhow::Result<()> {
    // log::info!("get_workspace_file_symbols {:?}", doc_id);
    if let Some(request_fut) = language_server.document_symbols(doc_id.clone()) {
      tokio::spawn(async move {
        request_fut
          .then(|response_json_result| async move {
            let response_json = response_json_result.unwrap();
            let response_parsed: Option<lsp::DocumentSymbolResponse> =
              serde_json::from_value(response_json)?;

            match response_parsed {
              Some(lsp::DocumentSymbolResponse::Nested(symbols)) => {
                // log::info!("nested symbols: {:#?}", symbols);
                // workspace_file.update_symbols(doc_symbols).unwrap();
                // log::debug!(
                //   "workspace_file symbols: {:#?}",
                //   workspace_file.file_tree
                // );
                tx.send(LsiAction::UpdateWorkspaceFileSymbols(workspace_path, doc_id, symbols))
                  .unwrap();
                // let mut flat_symbols = Vec::new();
                // for symbol in symbols {
                //   nested_to_flat(&mut flat_symbols, &doc_id, symbol, offset_encoding)
                // }
                // flat_symbols
                Ok(())
              },
              Some(lsp::DocumentSymbolResponse::Flat(_symbols)) => {
                // log::info!("flat symbols: {:#?}", _symbols);
                // symbols.into_iter().map(|symbol| SymbolInformationItem { symbol, offset_encoding }).collect()
                Err(anyhow::anyhow!("nested document symbol support is required"))
              },
              None => {
                log::info!("document symbol response is None");
                Err(anyhow::anyhow!("document symbol response is None"))
              },
            }
          })
          .await
      });
    };
    Ok(())
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

  pub async fn wait_for_progress_token_completion(&self, ids: &[usize]) -> anyhow::Result<()> {
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
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(1000));
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

  pub fn language_configuration_by_name(&self, name: &str) -> Option<Arc<LanguageConfiguration>> {
    self.loader.load().language_config_for_name(name)
  }

  pub async fn language_server_by_name(
    &self,
    language_server_name: String,
  ) -> Option<Arc<helix_lsp::Client>> {
    let client =
      self.language_servers.iter_clients().find(|client| client.name() == language_server_name);
    client.cloned()
  }

  pub fn language_server_by_id(&self, language_server_id: usize) -> Option<Arc<helix_lsp::Client>> {
    let client =
      self.language_servers.iter_clients().find(|client| client.id() == language_server_id);
    client.cloned()
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
