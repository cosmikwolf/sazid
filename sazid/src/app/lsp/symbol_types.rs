use std::cell::RefCell;
use std::fmt::{self, Display};
use std::path::PathBuf;

use helix_core::syntax::{FileType, LanguageConfiguration};
use lsp_types as lsp;
use url::Url;

use helix_lsp::Client;
use std::sync::{Arc, Mutex, Weak};

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
  pub file_tree: Arc<SourceSymbol>,
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

#[derive(Debug)]
pub struct SourceSymbol {
  pub name: String,
  pub detail: Option<String>,
  pub kind: lsp::SymbolKind,
  pub tags: Option<Vec<lsp::SymbolTag>>,
  pub range: lsp::Range,
  pub selection_range: lsp::Range,
  pub parent: Mutex<Weak<SourceSymbol>>,         // Wrap Weak in Mutex
  pub children: RefCell<Vec<Arc<SourceSymbol>>>, // Changed Rc to Arc
  pub uri: Url,
}

// impl Send for SourceSymbol {
//   fn poll_ready(
//     self: std::pin::Pin<&mut Self>,
//     cx: &mut std::task::Context<'_>,
//   ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
//     todo!()
//   }
//   fn start_send(
//     self: std::pin::Pin<&mut Self>,
//     item: std::result::Result<(), std::io::Error>,
//   ) -> std::result::Result<(), std::io::Error> {
//     todo!()
//   }
//   fn poll_flush(
//     self: std::pin::Pin<&mut Self>,
//     cx: &mut std::task::Context<'_>,
//   ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
//     todo!()
//   }
//   fn poll_close(
//     self: std::pin::Pin<&mut Self>,
//     cx: &mut std::task::Context<'_>,
//   ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
//     todo!()
//   }
// }
impl SourceSymbol {
  pub fn new(
    name: String,
    detail: Option<String>,
    kind: lsp::SymbolKind,
    tags: Option<Vec<lsp::SymbolTag>>,
    range: lsp::Range,
    selection_range: lsp::Range,
    uri: Url,
  ) -> Arc<Self> {
    // Changed Rc to Arc
    Arc::new(SourceSymbol {
      name,
      detail,
      kind,
      tags,
      range,
      selection_range,
      uri,
      parent: Mutex::new(Weak::new()), // Initialize Mutex
      children: RefCell::new(vec![]),
    })
  }

  pub fn new_file_symbol(uri: Url) -> Arc<Self> {
    // Changed Rc to Arc
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

  pub fn update_range(&mut self, new_range: lsp::Range) {
    self.range = new_range;
  }

  pub fn add_child(parent: Arc<SourceSymbol>, child: Arc<SourceSymbol>) {
    let mut parent_weak = child.parent.lock().unwrap();
    let child_arc = Arc::clone(&parent);
    *parent_weak = Arc::downgrade(&child_arc);
    // Add the child to the parent's children
    parent.children.borrow_mut().push(Arc::clone(&child));
  }

  pub fn set_parent(parent: Arc<SourceSymbol>, child: Arc<SourceSymbol>) {
    let mut parent_weak = child.parent.lock().unwrap();
    *parent_weak = Arc::downgrade(&parent);
  }

  pub fn iter_tree(arc_self: Arc<Self>) -> impl Iterator<Item = Arc<SourceSymbol>> {
    // Changed Rc to Arc
    // Initialize state for the iterator: a stack for DFS
    let mut stack: Vec<Arc<SourceSymbol>> = vec![arc_self];

    std::iter::from_fn(move || {
      if let Some(node) = stack.pop() {
        // When visiting a node, add its children to the stack for later visits
        let children = node.children.borrow();
        for child in children.iter().rev() {
          stack.push(Arc::clone(child));
        }
        Some(Arc::clone(&node))
      } else {
        None // When the stack is empty, iteration ends
      }
    })
  }

  pub fn from_document_symbol(
    doc_sym: &lsp::DocumentSymbol,
    file_uri: &Url,
    parent: Option<Arc<SourceSymbol>>,
  ) -> Arc<Self> {
    // Changed Rc to Arc
    let converted = SourceSymbol::new(
      doc_sym.name.clone(),
      doc_sym.detail.clone(),
      doc_sym.kind,
      doc_sym.tags.clone(),
      doc_sym.range,
      doc_sym.selection_range,
      file_uri.clone(),
    );

    if let Some(parent) = parent {
      Self::set_parent(parent, converted.clone());
    }

    if let Some(children) = doc_sym.children.clone() {
      for child in children {
        Self::from_document_symbol(&child, file_uri, Some(converted.clone()));
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
