use super::query::LsiQuery;
use super::symbol_types::SourceSymbol;
use super::workspace_file::WorkspaceFile;
use helix_core::syntax::{FileType, LanguageConfiguration};
use helix_lsp::Client;
use lsp_types::{DocumentSymbol, TextDocumentIdentifier};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

#[derive(Debug)]
pub struct Workspace {
  pub files: Vec<WorkspaceFile>,
  pub workspace_path: PathBuf,
  pub language_id: String,
  pub language_server: Arc<Client>,
  pub language_config: Arc<LanguageConfiguration>,
}

impl Workspace {
  pub fn new(
    workspace_path: &Path,
    language_id: String,
    language_server: Arc<Client>,
    language_config: Arc<LanguageConfiguration>,
  ) -> Self {
    Workspace {
      files: vec![],
      workspace_path: workspace_path.to_path_buf(),
      language_id,
      language_server,
      language_config,
    }
  }

  pub fn replace_doc_symbols(
    &mut self,
    doc_id: TextDocumentIdentifier,
    doc_symbols: Vec<DocumentSymbol>,
  ) -> anyhow::Result<()> {
    self
      .files
      .iter_mut()
      .find(|file| doc_id.uri.to_file_path().unwrap() == file.file_path)
      .expect("uri not found in workspace")
      .update_symbols(doc_symbols)
  }

  pub fn scan_workspace_files(&mut self) -> anyhow::Result<()> {
    let file_types = &self.language_config.file_types;
    self.files.extend(
      walkdir::WalkDir::new(&self.workspace_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter(|file_path| !self.files.iter().any(|f| f.file_path == file_path.path()))
        .filter(|e| {
          file_types.iter().any(|file_type| match file_type {
            FileType::Extension(file_type) => {
              e.path().extension().unwrap_or_default().to_str().unwrap() == file_type
            },
            FileType::Glob(glob) => {
              let matcher = glob.compile_matcher();
              matcher.is_match(e.path())
            },
          })
        })
        .flat_map(|e| e.path().canonicalize())
        .map(|file_path| {
          WorkspaceFile::new(
            &file_path,
            &self.workspace_path,
            &self.language_server.offset_encoding(),
          )
        })
        .collect::<Vec<WorkspaceFile>>(),
    );
    // clean up files that no longer exist
    self.files.retain(|f| f.file_path.exists());
    Ok(())
  }

  pub fn get_mut_file(&mut self, file_path: &Path) -> Option<&mut WorkspaceFile> {
    self.files.iter_mut().find(|f| f.file_path == file_path)
  }

  pub fn query_symbol_by_id(&self, symbol_id: &[u8; 32]) -> Option<Arc<SourceSymbol>> {
    self.all_symbols_weak().iter().map(|s| s.upgrade().unwrap()).find(|s| &s.symbol_id == symbol_id)
  }

  pub fn query_symbols(&self, query: &LsiQuery) -> anyhow::Result<Vec<Arc<SourceSymbol>>> {
    log::info!(
      "query: {:#?}\nsymbolcount: {}\nupgradeable symbols: {}",
      query,
      self.all_symbols_weak().len(),
      self.all_symbols_weak().iter().flat_map(|s| s.upgrade()).count()
    );

    if let Some(regex) = &query.file_path_regex {
      let regex = regex::Regex::new(regex).unwrap();

      if !self.files.iter().any(|f| {
        let file_path = f.file_path.to_str().unwrap();
        log::warn!("\nfile_path: {:?}\nregex: {:?}", file_path, regex);
        regex.is_match(file_path)
      }) {
        return Err(anyhow::anyhow!("no files match the provided regex\nregex: {:?}", regex));
      }
    }

    let symbols =
      self
        .all_symbols_weak()
        .iter()
        .flat_map(|s| s.upgrade())
        .filter(|s| {
          if let Some(file_name) = &query.file_path_regex {
            s.file_path.file_name().unwrap().to_str().unwrap() == file_name
              || &s.file_path.display().to_string() == file_name
          } else {
            true
          }
        })
        .filter(|s| {
          if let Some(name) = query.name_regex.clone() {
            s.name.contains(&name)
          } else {
            true
          }
        })
        .filter(|s| if let Some(kind) = query.kind { s.kind == kind } else { true })
        .filter(|s| {
          if let Some(range) = query.range {
            *s.range.lock().unwrap() == range
          } else {
            true
          }
        })
        .collect::<Vec<_>>();
    Ok(symbols)
  }

  pub fn all_symbols_weak(&self) -> Vec<Weak<SourceSymbol>> {
    let mut all_symbols = vec![];
    for file in &self.files {
      all_symbols.extend(file.symbol_list.iter().cloned());
      // log::info!("file: {:#?}", file.file_path);
    }
    // for symbol in all_symbols.iter() {
    //   log::info!("symbol: {:#?}", symbol.upgrade());
    // }
    all_symbols
  }

  pub fn count_symbols(&self) -> usize {
    self.all_symbols_weak().len()
  }
}
