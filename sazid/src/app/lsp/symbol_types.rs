use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::fmt::{self, Display};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use helix_core::syntax::{FileType, LanguageConfiguration};
use lsp_types as lsp;
use url::Url;

use helix_lsp::Client;
use std::sync::Arc;

pub struct Workspace {
  pub files: Vec<WorkspaceFile>,
  pub workspace_path: PathBuf,
  pub language_server: Arc<Client>,
  pub language_config: Arc<LanguageConfiguration>,
}

impl Workspace {
  pub fn new(
    workspace_path: PathBuf,
    language_server: Arc<Client>,
    language_config: Arc<LanguageConfiguration>,
  ) -> Self {
    let files = walkdir::WalkDir::new(&workspace_path)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|e| e.path().is_file())
      .filter(|e| e.path().extension().unwrap_or_default() == "rs")
      .flat_map(|e| e.path().canonicalize())
      .map(|file| Url::from_file_path(file).unwrap())
      .map(WorkspaceFile::new)
      .collect();
    Workspace { files, workspace_path, language_server, language_config }
  }

  async fn update_file_symbols(&mut self) -> anyhow::Result<()> {
    for file in self.files.iter_mut() {
      if file.needs_update()? {
        file.update_file_symbols(&self.language_server).await?;
        file.checksum = Some(file.get_checksum()?);
      }
    }
    Ok(())
  }

  pub async fn scan_workspace_files(&mut self) -> anyhow::Result<()> {
    let file_types = &self.language_config.file_types;
    self.files.extend(
      walkdir::WalkDir::new(&self.workspace_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter(|e| {
          file_types.iter().any(|file_type| match file_type {
            FileType::Extension(file_type) => e.path().extension().unwrap_or_default().to_str().unwrap() == file_type,
            FileType::Suffix(file_type) => e.path().ends_with(file_type),
          })
        })
        .flat_map(|e| e.path().canonicalize())
        .map(|file| Url::from_file_path(file).unwrap())
        .filter(|file_uri| !self.files.iter().any(|f| f.uri == *file_uri))
        .map(WorkspaceFile::new)
        .collect::<Vec<WorkspaceFile>>(),
    );

    Ok(())
  }
}

pub struct WorkspaceFile {
  pub file_tree: Rc<SourceSymbol>,
  pub uri: Url,
  pub checksum: Option<blake3::Hash>,
  pub version: i32,
}

impl WorkspaceFile {
  pub fn new(uri: Url) -> Self {
    let version = 0;
    let file_tree = SourceSymbol::new_file_symbol(uri.clone());
    WorkspaceFile { file_tree, uri, checksum: None, version }
  }
}

impl WorkspaceFile {
  fn get_checksum(&self) -> anyhow::Result<blake3::Hash> {
    let contents = std::fs::read(self.uri.path()).unwrap();
    Ok(blake3::hash(contents.as_slice()))
  }

  pub fn needs_update(&mut self) -> anyhow::Result<bool> {
    let new_checksum = self.get_checksum()?;
    if let Some(checksum) = self.checksum {
      if checksum == new_checksum {
        // if checksums match, then there is no need to update symbols
        return Ok(false);
      }
    }
    Ok(true)
  }

  pub async fn update_file_symbols(&mut self, language_server: &Arc<Client>) -> anyhow::Result<()> {
    let doc_id = lsp::TextDocumentIdentifier::new(self.uri.clone());
    self.file_tree = SourceSymbol::new_file_symbol(self.uri.clone());
    if let Some(request) = language_server.document_symbols(doc_id.clone()) {
      let response_json = request.await.unwrap();
      let response_parsed: Option<lsp::DocumentSymbolResponse> = serde_json::from_value(response_json)?;

      let _ = match response_parsed {
        Some(lsp::DocumentSymbolResponse::Nested(symbols)) => {
          symbols
          // let mut flat_symbols = Vec::new();
          // for symbol in symbols {
          //   nested_to_flat(&mut flat_symbols, &doc_id, symbol, offset_encoding)
          // }
          // flat_symbols
        },
        Some(lsp::DocumentSymbolResponse::Flat(_symbols)) => {
          // symbols.into_iter().map(|symbol| SymbolInformationItem { symbol, offset_encoding }).collect()
          return Err(anyhow::anyhow!("document symbol support is required"));
        },
        None => return Err(anyhow::anyhow!("document symbol response is None")),
      }
      .iter()
      .map(|s| SourceSymbol::from_document_symbol(s, &self.uri, Some(self.file_tree.clone())));
    };

    Ok(())
  }
}

#[derive(Debug, Clone)]
pub struct SourceSymbol {
  pub name: String,
  pub detail: Option<String>,
  pub kind: lsp::SymbolKind,
  pub tags: Option<Vec<lsp::SymbolTag>>,
  pub range: RefCell<lsp::Range>,
  pub selection_range: RefCell<lsp::Range>,
  pub parent: RefCell<Weak<SourceSymbol>>,
  pub children: RefCell<Vec<Rc<SourceSymbol>>>,
  pub uri: Url,
}

impl SourceSymbol {
  pub fn new(
    name: String,
    detail: Option<String>,
    kind: lsp::SymbolKind,
    tags: Option<Vec<lsp::SymbolTag>>,
    range: lsp::Range,
    selection_range: lsp::Range,
    uri: Url,
  ) -> Rc<Self> {
    Rc::new(SourceSymbol {
      name,
      detail,
      kind,
      tags,
      range: RefCell::new(range),
      selection_range: RefCell::new(selection_range),
      uri,
      parent: RefCell::new(Weak::new()),
      children: RefCell::new(vec![]),
    })
  }

  pub fn new_file_symbol(uri: Url) -> Rc<Self> {
    SourceSymbol::new(
      uri.to_string(),
      None,
      lsp::SymbolKind::FILE,
      None,
      lsp::Range {
        start: lsp_types::Position { line: 0, character: 0 },
        end: lsp_types::Position { line: 0, character: 0 },
      },
      lsp::Range {
        start: lsp_types::Position { line: 0, character: 0 },
        end: lsp_types::Position { line: 0, character: 0 },
      },
      uri,
    )
  }

  pub fn add_child(parent: &mut Rc<Self>, child: &Rc<SourceSymbol>) {
    *child.parent.borrow_mut() = Rc::downgrade(parent);
    parent.children.borrow_mut().push(Rc::clone(child));
    if parent.kind == lsp::SymbolKind::FILE && position_gt(child.range.borrow().end, parent.range.borrow().end) {
      let new_range = child.range.borrow().to_owned();
      *parent.range.borrow_mut() = new_range;
    }
  }

  pub fn iter_tree(rc_self: Rc<Self>) -> impl Iterator<Item = Rc<SourceSymbol>> {
    // Initialize state for the iterator: a stack for DFS
    let mut stack: Vec<Rc<SourceSymbol>> = vec![rc_self];

    std::iter::from_fn(move || {
      if let Some(node) = stack.pop() {
        // When visiting a node, add its children to the stack for later visits
        let children = node.children.borrow();
        for child in children.iter().rev() {
          stack.push(Rc::clone(child));
        }
        Some(Rc::clone(&node))
      } else {
        None // When the stack is empty, iteration ends
      }
    })
  }

  pub fn from_document_symbol(
    doc_sym: &lsp::DocumentSymbol,
    file_uri: &Url,
    parent: Option<Rc<SourceSymbol>>,
  ) -> Rc<Self> {
    let converted = SourceSymbol::new(
      doc_sym.name.clone(),
      doc_sym.detail.clone(),
      doc_sym.kind,
      doc_sym.tags.clone(),
      doc_sym.range,
      doc_sym.selection_range,
      file_uri.clone(),
    );

    if let Some(mut parent) = parent {
      SourceSymbol::add_child(&mut parent, &converted);
    }

    if let Some(children) = &doc_sym.children {
      for child in children {
        Self::from_document_symbol(child, file_uri, Some(Rc::clone(&converted)));
      }
    }

    converted
  }
}

impl Display for SourceSymbol {
  fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
    let filename = PathBuf::from(self.uri.path());
    let filename = filename.file_name().unwrap().to_str().unwrap();
    write!(f, "{:?} - {:?}: {}", filename, self.kind, self.name)?;
    let childcount = self.children.borrow().len();
    if childcount > 0 {
      write!(f, " ({} child nodes)", childcount)?;
    }
    Ok(())
  }
}

fn position_gt(pos1: lsp::Position, pos2: lsp::Position) -> bool {
  if pos1.line > pos2.line {
    true
  } else {
    pos1.line == pos2.line && pos1.character > pos2.character
  }
}
