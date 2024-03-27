use super::symbol_types::{DocumentChange, SourceSymbol};
use lsp_types as lsp;
use ropey::Rope;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use url::Url;

#[derive(Debug)]
pub struct WorkspaceFile {
  pub file_tree: Arc<SourceSymbol>,
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
    let file_tree = Arc::new(SourceSymbol::default());
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

  pub fn update_contents(&mut self) -> anyhow::Result<DocumentChange> {
    self.version += 1;
    self.checksum = Some(self.get_checksum()?);
    self.contents.insert(
      self.version,
      Rope::from_str(&std::fs::read_to_string(&self.file_path)?),
    );
    Ok(DocumentChange {
      original_contents: self.get_previous_version_contents(),
      new_contents: self.get_current_contents(),
      versioned_doc_id: lsp::VersionedTextDocumentIdentifier {
        uri: Url::from_file_path(&self.file_path).unwrap(),
        version: self.version,
      },
    })
  }

  pub fn update_symbols(
    &mut self,
    doc_symbols: Vec<lsp::DocumentSymbol>,
  ) -> anyhow::Result<()> {
    self.file_tree = Arc::new(
      SourceSymbol {
        name: self
          .file_path
          .strip_prefix(self.workspace_path.canonicalize().unwrap())
          .unwrap()
          .display()
          .to_string(),
        detail: None,
        kind: lsp::SymbolKind::FILE,
        tags: None,
        range: Arc::new(Mutex::new(lsp::Range {
          start: lsp_types::Position { line: 0, character: 0 },
          end: lsp_types::Position { line: 0, character: 0 },
        })),
        selection_range: Arc::new(Mutex::new(lsp::Range {
          start: lsp_types::Position { line: 0, character: 0 },
          end: lsp_types::Position { line: 0, character: 0 },
        })),
        file_path: self.file_path.to_path_buf(),
        parent: Arc::new(Mutex::new(Weak::new())),
        children: Arc::new(Mutex::new(vec![])),
        workspace_path: self.workspace_path.to_path_buf(),
        symbol_id: [0; 32],
      }
      .compute_hash(),
    );
    for symbol in doc_symbols {
      SourceSymbol::from_document_symbol(
        &symbol,
        &self.file_path,
        &mut self.file_tree,
        &mut self.symbol_list,
        &self.workspace_path,
      );
    }
    Ok(())
  }
}
