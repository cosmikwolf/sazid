use super::{get_file_range_contents, position_gt};
use lsp_types as lsp;
use ropey::Rope;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SymbolQuery {
  pub name: Option<String>,
  pub kind: Option<lsp::SymbolKind>,
  pub range: Option<lsp::Range>,
  pub file_name: Option<String>,
}

#[derive(Debug)]
pub struct DocumentChange {
  pub original_contents: Option<Rope>,
  pub new_contents: Rope,
  pub versioned_doc_id: lsp::VersionedTextDocumentIdentifier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSymbol {
  pub name: String,
  pub detail: Option<String>,
  pub kind: lsp::SymbolKind,
  pub tags: Option<Vec<lsp::SymbolTag>>,
  pub range: Arc<Mutex<lsp::Range>>,
  pub selection_range: Arc<Mutex<lsp::Range>>,
  pub parent: Arc<Mutex<Weak<SourceSymbol>>>,
  pub children: Arc<Mutex<Vec<Arc<SourceSymbol>>>>,
  pub workspace_path: PathBuf,
  pub file_path: PathBuf,
}

#[derive(Serialize)]
pub struct SerializableSourceSymbol {
  pub name: String,
  pub detail: Option<String>,
  pub kind: lsp::SymbolKind,
  pub tags: Option<Vec<lsp::SymbolTag>>,
  pub range: lsp::Range,
  pub selection_range: lsp::Range,
  pub workspace_path: PathBuf,
  pub file_path: PathBuf,
}

impl From<Arc<SourceSymbol>> for SerializableSourceSymbol {
  fn from(symbol: Arc<SourceSymbol>) -> Self {
    let symbol = symbol.as_ref();
    SerializableSourceSymbol {
      name: symbol.name.clone(),
      detail: symbol.detail.clone(),
      kind: symbol.kind,
      tags: symbol.tags.clone(),
      range: *symbol.range.lock().unwrap(),
      selection_range: *symbol.selection_range.lock().unwrap(),
      workspace_path: symbol.workspace_path.clone(),
      file_path: symbol.file_path.clone(),
    }
  }
}

impl Default for SourceSymbol {
  fn default() -> Self {
    SourceSymbol {
      kind: lsp::SymbolKind::FILE,
      name: String::new(),
      detail: None,
      tags: None,
      range: Arc::new(Mutex::new(lsp::Range {
        start: lsp_types::Position { line: 0, character: 0 },
        end: lsp_types::Position { line: 0, character: 0 },
      })),
      selection_range: Arc::new(Mutex::new(lsp::Range {
        start: lsp_types::Position { line: 0, character: 0 },
        end: lsp_types::Position { line: 0, character: 0 },
      })),
      parent: Arc::new(Mutex::new(Weak::new())),
      children: Arc::new(Mutex::new(Vec::new())),
      workspace_path: PathBuf::new(),
      file_path: PathBuf::new(),
    }
  }
}

impl SourceSymbol {
  pub fn from_document_symbol(
    doc_sym: &lsp::DocumentSymbol,
    file_path: &Path,
    parent: &mut Arc<SourceSymbol>,
    all_symbols: &mut Vec<Weak<SourceSymbol>>,
    workspace_path: &Path,
  ) -> Arc<Self> {
    let converted = Arc::new(SourceSymbol {
      name: doc_sym.name.clone(),
      detail: doc_sym.detail.clone(),
      kind: doc_sym.kind,
      tags: doc_sym.tags.clone(),
      range: Arc::new(Mutex::new(doc_sym.range)),
      selection_range: Arc::new(Mutex::new(doc_sym.selection_range)),
      file_path: file_path.to_path_buf(),
      parent: Arc::new(Mutex::new(Weak::new())),
      children: Arc::new(Mutex::new(vec![])),
      workspace_path: workspace_path.to_path_buf(),
    });
    all_symbols.push(Arc::downgrade(&converted));
    SourceSymbol::add_child(parent, &converted);
    if let Some(children) = &doc_sym.children {
      for child in children {
        Self::from_document_symbol(
          child,
          file_path,
          &mut Arc::clone(&converted),
          all_symbols,
          workspace_path,
        );
      }
    }
    converted
  }

  pub fn get_source(&self) -> anyhow::Result<String> {
    let file_path = &self.file_path;
    let range = self.range.lock().unwrap();
    get_file_range_contents(file_path, *range)
  }

  pub fn get_selection(&self) -> anyhow::Result<String> {
    let file_path = &self.file_path;
    let range = self.selection_range.lock().unwrap();
    get_file_range_contents(file_path, *range)
  }

  pub fn add_child(parent: &mut Arc<Self>, child: &Arc<SourceSymbol>) {
    *child.parent.lock().unwrap() = Arc::downgrade(parent);
    parent.children.lock().unwrap().push(Arc::clone(child));
    if parent.kind == lsp::SymbolKind::FILE
      && position_gt(
        child.range.lock().unwrap().end,
        parent.range.lock().unwrap().end,
      )
    {
      let new_range = lsp::Range {
        start: parent.range.lock().unwrap().start,
        end: child.range.lock().unwrap().end,
      };
      *parent.range.lock().unwrap() = new_range;
    }
  }
}

impl Display for SourceSymbol {
  fn fmt(
    &self,
    f: &mut fmt::Formatter,
  ) -> std::result::Result<(), std::fmt::Error> {
    let filename = PathBuf::from(&self.file_path);
    let filename = filename.file_name().unwrap().to_str().unwrap();
    write!(f, "{:?} - {:?}: {}", filename, self.kind, self.name)?;
    let childcount = self.children.lock().unwrap().len();
    if childcount > 0 {
      write!(f, " ({} child nodes)", childcount)?;
    }
    Ok(())
  }
}
