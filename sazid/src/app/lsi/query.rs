use std::path::PathBuf;

use helix_lsp::lsp;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticIncludeFlags {
  pub include_errors: Option<bool>,
  pub include_warnings: Option<bool>,
  pub include_information: Option<bool>,
  pub include_hints: Option<bool>,
  pub include_no_severity: Option<bool>,
}

#[derive(Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LsiQuery {
  pub name_regex: Option<String>,
  pub file_path_regex: Option<String>,
  pub kind: Option<lsp::SymbolKind>,
  pub range: Option<lsp::Range>,
  pub other_regex: Option<String>,
  pub diagnostic_severity: Option<DiagnosticIncludeFlags>,
  pub symbol_id: Option<Vec<u8>>,
  pub workspace_root: PathBuf,
  pub session_id: i64,
  pub tool_call_id: String,
  pub test_query: bool,
}
