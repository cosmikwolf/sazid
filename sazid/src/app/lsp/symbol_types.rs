use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use helix_core::syntax::{FileType, LanguageConfiguration};
use lsp_types as lsp;
use ropey::Rope;
use url::Url;

use std::sync::Arc;

pub struct Workspace {
  pub files: Vec<WorkspaceFile>,
  pub workspace_path: PathBuf,
  pub language_id: String,
  pub language_server_id: usize,
  pub language_config: Arc<LanguageConfiguration>,
  pub offset_encoding: helix_lsp::OffsetEncoding,
}

pub struct SymbolQuery {
  name: Option<String>,
  kind: Option<lsp::SymbolKind>,
  range: Option<lsp::Range>,
  file: Option<String>,
}

impl Workspace {
  pub fn new(
    workspace_path: &Path,
    language_id: String,
    language_server_id: usize,
    language_config: Arc<LanguageConfiguration>,
    offset_encoding: helix_lsp::OffsetEncoding,
  ) -> Self {
    Workspace {
      files: vec![],
      workspace_path: workspace_path.to_path_buf(),
      language_id,
      language_server_id,
      language_config,
      offset_encoding,
    }
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
            FileType::Extension(file_type) => {
              e.path().extension().unwrap_or_default().to_str().unwrap()
                == file_type
            },
            FileType::Suffix(file_type) => e.path().ends_with(file_type),
          })
        })
        .flat_map(|e| e.path().canonicalize())
        .map(|file_path| {
          WorkspaceFile::new(
            &file_path,
            &self.workspace_path,
            &self.offset_encoding,
          )
        })
        .collect::<Vec<WorkspaceFile>>(),
    );
    // clean up files that no longer exist
    self.files.retain(|f| f.file_path.exists());
    Ok(())
  }

  pub fn get_mut_file(
    &mut self,
    file_path: &Path,
  ) -> Option<&mut WorkspaceFile> {
    self.files.iter_mut().find(|f| f.file_path == file_path)
  }

  pub async fn query_symbols(
    &self,
    query: SymbolQuery,
  ) -> anyhow::Result<Vec<Rc<SourceSymbol>>> {
    Ok(
      self
        .all_symbols_weak()
        .iter()
        .map(|s| s.upgrade().unwrap())
        .filter(|s| {
          if let Some(file_name) = &query.file {
            s.file_path.file_name().unwrap().to_str().unwrap() == file_name
              || &s
                .file_path
                .strip_prefix(&self.workspace_path)
                .unwrap()
                .display()
                .to_string()
                == file_name
          } else {
            false
          }
        })
        .filter(|s| {
          if let Some(name) = query.name.clone() {
            s.name == name
          } else {
            false
          }
        })
        .filter(|s| {
          if let Some(kind) = query.kind {
            s.kind == kind
          } else {
            false
          }
        })
        .filter(|s| {
          if let Some(range) = query.range {
            *s.range.borrow() == range
          } else {
            false
          }
        })
        .collect::<Vec<_>>(),
    )
  }

  pub fn all_symbols_weak(&self) -> Vec<Weak<SourceSymbol>> {
    let mut all_symbols = vec![];
    for file in &self.files {
      all_symbols.extend(file.symbol_list.iter().cloned());
      log::info!("file: {:#?}", file.file_path);
    }
    all_symbols
  }

  pub fn count_symbols(&self) -> usize {
    self.all_symbols_weak().len()
  }
}

#[derive(Debug)]
pub struct DocumentChange {
  pub original_contents: Option<Rope>,
  pub new_contents: Rope,
  pub versioned_doc_id: lsp::VersionedTextDocumentIdentifier,
}

pub struct WorkspaceFile {
  pub file_tree: Rc<SourceSymbol>,
  pub symbol_list: Vec<Weak<SourceSymbol>>,
  pub file_path: PathBuf,
  pub diagnostics: HashMap<i32, Vec<lsp::Diagnostic>>,
  pub checksum: Option<blake3::Hash>,
  pub contents: HashMap<i32, Rope>, // hashmap of contents indexed by version
  pub offset_encoding: helix_lsp::OffsetEncoding,
  pub workspace_path: PathBuf,
  pub version: i32,
}

impl WorkspaceFile {
  pub fn new(
    file_path: &Path,
    workspace_path: &Path,
    offset_encoding: &helix_lsp::OffsetEncoding,
  ) -> Self {
    let version = 0;
    let file_tree = Rc::new(SourceSymbol::default());
    WorkspaceFile {
      file_tree,
      symbol_list: vec![],
      file_path: file_path.to_path_buf(),
      diagnostics: HashMap::new(),
      checksum: None,
      offset_encoding: *offset_encoding,
      contents: HashMap::new(),
      version,
      workspace_path: workspace_path.to_path_buf(),
    }
  }
}

impl WorkspaceFile {
  pub fn get_current_contents(&self) -> Rope {
    self
      .contents
      .get(&self.version)
      .cloned()
      .expect("No contents found for current version")
  }

  pub fn get_previous_version_contents(&self) -> Option<Rope> {
    let previous_version = self.version - 1;
    self.contents.get(&previous_version).cloned()
  }

  fn get_checksum(&self) -> anyhow::Result<blake3::Hash> {
    let contents = std::fs::read(&self.file_path).unwrap();
    Ok(blake3::hash(contents.as_slice()))
  }

  pub fn get_text_document_id(
    &self,
  ) -> anyhow::Result<lsp::TextDocumentIdentifier> {
    Ok(lsp::TextDocumentIdentifier::new(
      Url::from_file_path(&self.file_path).unwrap(),
    ))
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

  pub fn update(
    &mut self,
    doc_symbols: Vec<lsp::DocumentSymbol>,
  ) -> anyhow::Result<DocumentChange> {
    self.version += 1;
    self.checksum = Some(self.get_checksum()?);
    self.contents.insert(
      self.version,
      Rope::from_str(&std::fs::read_to_string(&self.file_path)?),
    );
    self.file_tree = Rc::new(SourceSymbol {
      name: self
        .file_path
        .strip_prefix(self.workspace_path.canonicalize().unwrap())
        .unwrap()
        .display()
        .to_string(),
      detail: None,
      kind: lsp::SymbolKind::FILE,
      tags: None,
      range: RefCell::new(lsp::Range {
        start: lsp_types::Position { line: 0, character: 0 },
        end: lsp_types::Position { line: 0, character: 0 },
      }),
      selection_range: RefCell::new(lsp::Range {
        start: lsp_types::Position { line: 0, character: 0 },
        end: lsp_types::Position { line: 0, character: 0 },
      }),
      file_path: self.file_path.to_path_buf(),
      parent: RefCell::new(Weak::new()),
      children: RefCell::new(vec![]),
      workspace_path: self.workspace_path.to_path_buf(),
    });

    for symbol in doc_symbols {
      SourceSymbol::from_document_symbol(
        &symbol,
        &self.file_path,
        &mut self.file_tree,
        &self.workspace_path,
      );
    }

    Ok(DocumentChange {
      original_contents: self.get_previous_version_contents(),
      new_contents: self.get_current_contents(),
      versioned_doc_id: lsp::VersionedTextDocumentIdentifier {
        uri: Url::from_file_path(&self.file_path).unwrap(),
        version: self.version,
      },
    })
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

    SourceSymbol::add_child(parent, &converted);

    if let Some(children) = &doc_sym.children {
      for child in children {
        Self::from_document_symbol(
          child,
          file_path,
          &mut Rc::clone(&converted),
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

fn position_gt(pos1: lsp::Position, pos2: lsp::Position) -> bool {
  if pos1.line > pos2.line {
    true
  } else {
    pos1.line == pos2.line && pos1.character > pos2.character
  }
}

fn get_file_range_contents(
  file_path: &Path,
  range: lsp::Range,
) -> anyhow::Result<String> {
  let source_code = std::fs::read_to_string(file_path)?;
  if range.start == range.end {
    return Ok(String::new());
  }
  let source_code = source_code
    .lines()
    .skip(range.start.line as usize)
    .take((range.end.line - range.start.line) as usize + 1)
    .enumerate()
    .map(|(i, line)| {
      if i == 0 {
        line.chars().skip(range.start.character as usize).collect()
      } else if i == (range.end.line - range.start.line) as usize {
        line.chars().take(range.end.character as usize).collect()
      } else {
        line.to_string()
      }
    })
    .collect::<Vec<_>>()
    .join("\n");
  Ok(source_code)
}
