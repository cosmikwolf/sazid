use std::cell::RefCell;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use lsp_types as lsp;
use ropey::Rope;
use serde::{Deserialize, Serialize};

use super::{get_file_range_contents, position_gt};

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
  pub range: RefCell<lsp::Range>,
  pub selection_range: RefCell<lsp::Range>,
  pub parent: RefCell<Weak<SourceSymbol>>,
  pub children: RefCell<Vec<Rc<SourceSymbol>>>,
  pub workspace_path: PathBuf,
  pub file_path: PathBuf,
}

impl Default for SourceSymbol {
  fn default() -> Self {
    SourceSymbol {
      kind: lsp::SymbolKind::FILE,
      name: String::new(),
      detail: None,
      tags: None,
      range: RefCell::new(lsp::Range {
        start: lsp_types::Position { line: 0, character: 0 },
        end: lsp_types::Position { line: 0, character: 0 },
      }),
      selection_range: RefCell::new(lsp::Range {
        start: lsp_types::Position { line: 0, character: 0 },
        end: lsp_types::Position { line: 0, character: 0 },
      }),
      parent: RefCell::new(Weak::new()),
      children: RefCell::new(Vec::new()),
      workspace_path: PathBuf::new(),
      file_path: PathBuf::new(),
    }
  }
}

impl SourceSymbol {
  pub fn from_document_symbol(
    doc_sym: &lsp::DocumentSymbol,
    file_path: &Path,
    parent: &mut Rc<SourceSymbol>,
    all_symbols: &mut Vec<Weak<SourceSymbol>>,
    workspace_path: &Path,
  ) -> Rc<Self> {
    let converted = Rc::new(SourceSymbol {
      name: doc_sym.name.clone(),
      detail: doc_sym.detail.clone(),
      kind: doc_sym.kind,
      tags: doc_sym.tags.clone(),
      range: RefCell::new(doc_sym.range),
      selection_range: RefCell::new(doc_sym.selection_range),
      file_path: file_path.to_path_buf(),
      parent: RefCell::new(Weak::new()),
      children: RefCell::new(vec![]),
      workspace_path: workspace_path.to_path_buf(),
    });

    all_symbols.push(Rc::downgrade(&converted));
    SourceSymbol::add_child(parent, &converted);

    if let Some(children) = &doc_sym.children {
      for child in children {
        Self::from_document_symbol(
          child,
          file_path,
          &mut Rc::clone(&converted),
          all_symbols,
          workspace_path,
        );
      }
    }

    converted
  }

  pub fn get_source(&self) -> anyhow::Result<String> {
    let file_path = &self.file_path;
    let range = *self.range.borrow();
    get_file_range_contents(file_path, range)
  }

  pub fn get_selection(&self) -> anyhow::Result<String> {
    let file_path = &self.file_path;
    let range = *self.selection_range.borrow();
    get_file_range_contents(file_path, range)
  }

  pub fn add_child(parent: &mut Rc<Self>, child: &Rc<SourceSymbol>) {
    *child.parent.borrow_mut() = Rc::downgrade(parent);
    parent.children.borrow_mut().push(Rc::clone(child));
    if parent.kind == lsp::SymbolKind::FILE
      && position_gt(child.range.borrow().end, parent.range.borrow().end)
    {
      let new_range = lsp::Range {
        start: parent.range.borrow().start,
        end: child.range.borrow().end,
      };
      *parent.range.borrow_mut() = new_range;
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
    let childcount = self.children.borrow().len();
    if childcount > 0 {
      write!(f, " ({} child nodes)", childcount)?;
    }
    Ok(())
  }
}
