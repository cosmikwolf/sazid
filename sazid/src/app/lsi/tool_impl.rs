use std::path::PathBuf;

use futures_util::FutureExt;
use serde_json::json;

use super::workspace::Workspace;
use super::{
  interface::LanguageServerInterface, query::LsiQuery, symbol_types::SerializableSourceSymbol,
};
use sazid_lsp::lsp::{self};

use lsp::{Diagnostic, DiagnosticSeverity, NumberOrString};
use url::Url;

impl LanguageServerInterface {
  pub fn goto_type_definition(&self, lsi_query: &LsiQuery) -> anyhow::Result<()> {
    let workspace = self.get_workspace(lsi_query).unwrap();
    let symbol_id =
      TryInto::<[u8; 32]>::try_into(lsi_query.symbol_id.clone().expect("symbol_id not set"))
        .expect("symbol id has the incorrect number of bytes");
    let symbol = workspace
      .query_symbol_by_id(&symbol_id)
      .unwrap_or_else(|| panic!("could not find symbol with id {:#?}", symbol_id));
    let text_document =
      lsp::TextDocumentIdentifier { uri: Url::from_file_path(symbol.file_path.clone()).unwrap() };
    let position = symbol.selection_range.lock().unwrap().start;
    let work_done_token = Some(NumberOrString::String("goto type definition".to_string()));
    let response = workspace
      .language_server
      .goto_type_definition(text_document, position, work_done_token)
      .expect("could not obtain goto definition response");

    let lsi_query = lsi_query.clone();
    let tx = self.tx.clone();
    tokio::spawn(async move {
      let result = response.await;
      let result = result
        .map(|value: serde_json::Value| serde_json::to_string_pretty(&value))
        .unwrap()
        .unwrap();
      Self::send_query_response(&tx, lsi_query, Ok(result));
    });

    Ok(())
  }

  pub fn goto_symbol_definition(&self, lsi_query: &LsiQuery) -> anyhow::Result<()> {
    let workspace = self.get_workspace(lsi_query).unwrap();
    let symbol_id =
      TryInto::<[u8; 32]>::try_into(lsi_query.symbol_id.clone().expect("symbol_id not set"))
        .expect("symbol id has the incorrect number of bytes");
    let symbol = workspace
      .query_symbol_by_id(&symbol_id)
      .unwrap_or_else(|| panic!("could not find symbol with id {:#?}", symbol_id));
    let text_document =
      lsp::TextDocumentIdentifier { uri: Url::from_file_path(symbol.file_path.clone()).unwrap() };
    let position = symbol.selection_range.lock().unwrap().start;
    let work_done_token = Some(NumberOrString::String("goto definition".to_string()));
    let response = workspace
      .language_server
      .goto_definition(text_document, position, work_done_token)
      .expect("could not obtain goto definition response");

    let lsi_query = lsi_query.clone();
    let tx = self.tx.clone();
    tokio::spawn(async move {
      let result = response.await;
      let result = result
        .map(|value: serde_json::Value| serde_json::to_string_pretty(&value))
        .unwrap()
        .unwrap();
      Self::send_query_response(&tx, lsi_query, Ok(result));
    });

    Ok(())
  }

  pub fn goto_symbol_declaration(&self, lsi_query: &LsiQuery) -> anyhow::Result<()> {
    let workspace = self.get_workspace(lsi_query).unwrap();
    let symbol_id =
      TryInto::<[u8; 32]>::try_into(lsi_query.symbol_id.clone().expect("symbol_id not set"))
        .expect("symbol id has the incorrect number of bytes");
    let symbol = workspace
      .query_symbol_by_id(&symbol_id)
      .unwrap_or_else(|| panic!("could not find symbol with id {:#?}", symbol_id));
    let text_document =
      lsp::TextDocumentIdentifier { uri: Url::from_file_path(symbol.file_path.clone()).unwrap() };
    let position = symbol.selection_range.lock().unwrap().start;
    let work_done_token = Some(NumberOrString::String("goto declaration".to_string()));
    let response = workspace
      .language_server
      .goto_declaration(text_document, position, work_done_token)
      .expect("could not obtain goto declaration response");

    let lsi_query = lsi_query.clone();
    let tx = self.tx.clone();
    tokio::spawn(async move {
      let result = response.await;
      let result = result
        .map(|value: serde_json::Value| serde_json::to_string_pretty(&value))
        .unwrap()
        .unwrap();
      Self::send_query_response(&tx, lsi_query, Ok(result));
    });

    Ok(())
  }

  pub fn get_diagnostics(&self, lsi_query: &LsiQuery) -> anyhow::Result<String> {
    let workspace = self.get_workspace(lsi_query)?;

    let file_regex = lsi_query
      .file_path_regex
      .as_ref()
      .map(|pattern| regex::Regex::new(pattern).expect("invalid regex pattern"));

    let diagnostics = workspace
      .files
      .iter()
      .filter(|file| match file_regex.clone() {
        Some(file_regex) => file_regex.is_match(&file.file_path.display().to_string()),
        None => true,
      })
      .map(|file| {
        let diagnostics = file.diagnostics.get(&file.version).map(|d| {
          d.iter()
            .filter(|d| match lsi_query.diagnostic_severity {
              Some(ref s) => match d.severity {
                Some(DiagnosticSeverity::ERROR) => s.include_errors.unwrap_or(true),
                Some(DiagnosticSeverity::WARNING) => s.include_warnings.unwrap_or(true),
                Some(DiagnosticSeverity::INFORMATION) => s.include_information.unwrap_or(true),
                Some(DiagnosticSeverity::HINT) => s.include_hints.unwrap_or(true),
                Some(_) => s.include_no_severity.unwrap_or(true),
                None => s.include_no_severity.unwrap_or(true),
              },
              None => true,
            })
            .collect::<Vec<_>>()
        });
        (file.file_path.clone(), diagnostics)
      })
      .collect::<Vec<(PathBuf, Option<Vec<&Diagnostic>>)>>();
    Ok(json!(diagnostics).to_string())
  }

  pub fn get_workspace_files(&self, lsi_query: &LsiQuery) -> anyhow::Result<String> {
    let workspace = self.get_workspace(lsi_query)?;

    let pattern = lsi_query
      .file_path_regex
      .as_ref()
      .map(|pattern| regex::Regex::new(pattern).expect("invalid regex pattern"));

    match pattern {
      Some(pattern) => {
        let files = workspace
          .files
          .iter()
          .filter(|file| pattern.is_match(&file.file_path.display().to_string()))
          .map(|file| file.file_path.clone())
          .collect::<Vec<_>>();
        Ok(json!(files).to_string())
      },
      None => {
        let files = workspace.files.iter().map(|file| file.file_path.clone()).collect::<Vec<_>>();
        Ok(json!(files).to_string())
      },
    }
  }

  pub fn lsi_query_workspace_symbols(&mut self, lsi_query: &LsiQuery) -> anyhow::Result<String> {
    match self.get_workspace(lsi_query)?.query_symbols(lsi_query) {
      Ok(symbols) => match symbols.len() {
        0 => Ok("no symbols found".to_string()),
        _ => match serde_json::to_string(
          &symbols.into_iter().map(SerializableSourceSymbol::from).collect::<Vec<_>>(),
        ) {
          Ok(content) => Ok(content),
          Err(e) => Err(anyhow::anyhow!("error serializing symbols: {}", e)),
        },
      },
      Err(e) => Err(anyhow::anyhow!("error querying workspace symbols: {}", e)),
    }
  }

  fn get_workspace(&self, lsi_query: &LsiQuery) -> anyhow::Result<&Workspace> {
    match self.workspaces.iter().find(|w| w.workspace_path == lsi_query.workspace_root) {
      Some(workspace) => Ok(workspace),
      None => Err(anyhow::anyhow!("no workspace found at {:#?}", lsi_query.workspace_root)),
    }
  }
}
