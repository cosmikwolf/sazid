use std::cell::RefCell;
use std::fmt::{self, Display};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use helix_core::syntax::{FileType, LanguageConfiguration};
use lsp_types as lsp;
use url::Url;

use std::sync::Arc;

pub struct Workspace {
  pub files: Vec<WorkspaceFile>,
  pub workspace_path: PathBuf,
  // pub language_server: Arc<Client>,
  pub language_server_id: usize,
  pub language_config: Arc<LanguageConfiguration>,
}

pub struct SymbolQuery {
  name: Option<String>,
  kind: Option<lsp::SymbolKind>,
  range: Option<lsp::Range>,
  uri: Option<String>,
  file: Option<String>,
}

impl Workspace {
  pub fn new(
    workspace_path: PathBuf,
    // language_server: Arc<Client>,
    language_server_id: usize,
    language_config: Arc<LanguageConfiguration>,
  ) -> Self {
    let mut workspace =
      Workspace { files: vec![], workspace_path: workspace_path.clone(), language_server_id, language_config };
    let mut workspace_rc = Rc::new(workspace);
    workspace.files = walkdir::WalkDir::new(&workspace_path)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter(|e| e.path().is_file())
      .filter(|e| e.path().extension().unwrap_or_default() == "rs")
      .flat_map(|e| e.path().canonicalize())
      .map(|file| Url::from_file_path(file).unwrap())
      .map(|uri| WorkspaceFile::new(uri, &workspace_rc))
      .collect();
    workspace
  }

  pub fn scan_workspace_files(&mut self) -> anyhow::Result<()> {
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
        .map(|uri| WorkspaceFile::new(uri, &Rc::new(*self)))
        .collect::<Vec<WorkspaceFile>>(),
    );
    // clean up files that no longer exist
    self.files.retain(|f| f.uri.to_file_path().unwrap().exists());
    Ok(())
  }

  pub fn query_symbols(&self, query: SymbolQuery) -> Vec<Rc<SourceSymbol>> {
    self
      .files
      .iter()
      .flat_map(|f| {
        SourceSymbol::iter_tree(Rc::clone(&f.file_tree)).filter(|_s| {
          if let Some(file) = query.file.clone() {
            f.uri.to_string() == file
          } else {
            true
          }
        })
      })
      .filter(|s| if let Some(name) = query.name.clone() { s.name == name } else { true })
      .filter(|s| if let Some(kind) = query.kind { s.kind == kind } else { true })
      .filter(|s| if let Some(range) = query.range { *s.range.borrow() == range } else { true })
      .filter(|s| if let Some(uri) = query.uri.clone() { s.uri.to_string() == uri } else { true })
      .collect::<Vec<_>>()
  }

  pub fn iter_symbols(&self) -> impl Iterator<Item = Rc<SourceSymbol>> {
    self.files.iter().flat_map(|f| SourceSymbol::iter_tree(Rc::clone(&f.file_tree))).collect::<Vec<_>>().into_iter()
  }

  pub fn count_symbols(&self) -> usize {
    Workspace::iter_symbols(self).count()
  }
}

pub struct WorkspaceFile {
  pub file_tree: Rc<SourceSymbol>,
  pub uri: Url,
  pub checksum: Option<blake3::Hash>,
  pub workspace: RefCell<Weak<Workspace>>,
  pub version: i32,
}

impl WorkspaceFile {
  pub fn new(uri: Url, workspace: &Rc<Workspace>) -> Self {
    let version = 0;
    let file_tree = SourceSymbol::new_file_symbol(&uri, workspace);
    let workspace = RefCell::new(Rc::downgrade(workspace));
    WorkspaceFile { file_tree, uri, checksum: None, version, workspace }
  }
}

impl WorkspaceFile {
  fn get_checksum(&self) -> anyhow::Result<blake3::Hash> {
    let contents = std::fs::read(self.uri.path()).unwrap();
    Ok(blake3::hash(contents.as_slice()))
  }

  pub fn needs_update(&self) -> anyhow::Result<bool> {
    let new_checksum = self.get_checksum()?;
    if let Some(checksum) = self.checksum {
      if checksum == new_checksum {
        // if checksums match, then there is no need to update symbols
        return Ok(false);
      }
    }
    // If no checksum exists, or if they don't match, then an update is indicated
    Ok(true)
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
  pub workspace: RefCell<Weak<Workspace>>,
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
    workspace: &Rc<Workspace>,
  ) -> Rc<Self> {
    let workspace = RefCell::new(Rc::downgrade(workspace));

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
      workspace,
    })
  }

  pub fn get_symbol_source_code(&self) -> anyhow::Result<String> {
    let file_path = self.uri.path();
    let source_code = std::fs::read_to_string(file_path)?;
    let start = self.range.borrow().start;
    let end = self.range.borrow().end;
    let source_code = source_code
      .lines()
      .skip(start.line as usize)
      .take((end.line - start.line) as usize + 1)
      .enumerate()
      .map(|(i, line)| {
        if i == 0 {
          line.chars().skip(start.character as usize).collect()
        } else if i == (end.line - start.line) as usize {
          line.chars().take(end.character as usize).collect()
        } else {
          line.to_string()
        }
      })
      .collect::<Vec<_>>()
      .join("\n");
    Ok(source_code)
  }

  pub fn new_file_tree(uri: &Url, doc_symbols: Vec<lsp::DocumentSymbol>, workspace: &Rc<Workspace>) -> Rc<Self> {
    let file_tree = &mut Self::new_file_symbol(uri, workspace);
    for symbol in doc_symbols {
      SourceSymbol::from_document_symbol(&symbol, uri, file_tree, workspace);
    }
    file_tree.clone()
  }

  pub fn new_file_symbol(uri: &Url, workspace: &Rc<Workspace>) -> Rc<Self> {
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
      uri.clone(),
      workspace,
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
    parent: &mut Rc<SourceSymbol>,
    workspace: &Rc<Workspace>,
  ) -> Rc<Self> {
    let converted = SourceSymbol::new(
      doc_sym.name.clone(),
      doc_sym.detail.clone(),
      doc_sym.kind,
      doc_sym.tags.clone(),
      doc_sym.range,
      doc_sym.selection_range,
      file_uri.clone(),
      workspace,
    );

    SourceSymbol::add_child(parent, &converted);

    if let Some(children) = &doc_sym.children {
      for child in children {
        Self::from_document_symbol(child, file_uri, &mut Rc::clone(&converted), workspace);
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
