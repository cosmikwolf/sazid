use super::query::LsiQuery;
use super::symbol_types::SourceSymbol;
use super::workspace_file::WorkspaceFile;
use helix_core::syntax::{FileType, LanguageConfiguration};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

#[derive(Debug)]
pub struct Workspace {
  pub files: Vec<WorkspaceFile>,
  pub workspace_path: PathBuf,
  pub language_id: String,
  pub language_server_id: usize,
  pub language_config: Arc<LanguageConfiguration>,
  pub offset_encoding: helix_lsp::OffsetEncoding,
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

  pub fn query_symbol_by_id(
    &self,
    symbol_id: &[u8; 32],
  ) -> Option<Arc<SourceSymbol>> {
    let matches = self
      .all_symbols_weak()
      .iter()
      .map(|s| s.upgrade().unwrap())
      .filter(|s| &s.symbol_id == symbol_id)
      .collect::<Vec<_>>();

    match matches.len() {
      0 => None,
      1 => Some(matches[0].clone()),
      _ => panic!("multiple symbols with the same id found in workspace"),
    }
  }

  pub fn query_symbols(
    &self,
    query: &LsiQuery,
  ) -> anyhow::Result<Vec<Arc<SourceSymbol>>> {
    log::info!("query: {:#?}", query);
    Ok(
      self
        .all_symbols_weak()
        .iter()
        .map(|s| s.upgrade().unwrap())
        .filter(|s| {
          if let Some(file_name) = &query.file_path_regex {
            s.file_path.file_name().unwrap().to_str().unwrap() == file_name
              || &s
                .file_path
                .strip_prefix(&self.workspace_path)
                .unwrap()
                .display()
                .to_string()
                == file_name
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
        .filter(
          |s| {
            if let Some(kind) = query.kind {
              s.kind == kind
            } else {
              true
            }
          },
        )
        .filter(|s| {
          if let Some(range) = query.range {
            *s.range.lock().unwrap() == range
          } else {
            true
          }
        })
        .collect::<Vec<_>>(),
    )
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
