use helix_core;
use helix_lsp::Client;
use sazid::app::lsp::helix_lsp_interface::LanguageServerInterface;
use sazid::trace_dbg;
use sazid::utils::initialize_logging;
use std::path::Path;
use std::str::from_utf8;
use tempfile::tempdir;
use url::Url;

pub fn test_lang_config() -> helix_core::syntax::Configuration {
  let default_config = include_bytes!("./assets/languages_test.toml");
  toml::from_str::<helix_core::syntax::Configuration>(from_utf8(default_config).unwrap())
    .expect("Could not parse built-in languages.toml to valid toml")
}

fn copy_dir_recursively(source: &Path, target: &Path) -> anyhow::Result<()> {
  if source.is_dir() {
    if !target.exists() {
      std::fs::create_dir_all(target)?;
    }

    for entry in std::fs::read_dir(source)? {
      let entry = entry?;
      let path = entry.path();
      let target_path = target.join(entry.file_name());

      if path.is_dir() {
        copy_dir_recursively(&path, &target_path)?;
      } else {
        std::fs::copy(&path, &target_path)?;
      }
    }
  } else {
    std::fs::copy(source, target)?;
  }
  Ok(())
}

#[test]
fn test_logging() -> anyhow::Result<()> {
  // let res = initialize_logging();
  // assert!(res.is_ok());
  trace_dbg!("log test");
  Ok(())
}

#[tokio::test]
async fn test_rust_analyzer_connection() -> anyhow::Result<()> {
  let res = initialize_logging();
  assert!(res.is_ok());
  trace_dbg!("beginning workspace scan tests");

  let test_workspace_src_path = "tests/assets/rust_test_project";
  let test_src_assets = std::env::current_dir().unwrap().join(test_workspace_src_path);

  // create temp dir for test
  let temp_dir = tempdir()?;
  let test_workspace_path = temp_dir.into_path().join(test_workspace_src_path);

  println!("Test workspace path: {:#?}", test_workspace_path);
  // recursively copy test_src_assets into temp_dir
  copy_dir_recursively(&test_src_assets, &test_workspace_path).unwrap();

  assert!(test_workspace_path.exists());

  std::env::set_current_dir(&test_workspace_path).unwrap();
  let config = test_lang_config();
  let mut lsi = LanguageServerInterface::new(Some(config));
  let root_dirs = vec![test_workspace_path.clone()];

  lsi.create_workspace(test_workspace_path.clone(), "rust", "rust-analyzer", None).await.unwrap();
  let ids = lsi.language_servers.lock().await.iter_clients().map(|c| c.id()).collect::<Vec<usize>>();

  let a = lsi.wait_for_language_server_initialization(ids.as_slice()).await;
  assert!(a.is_ok());
  let a = lsi.update_workspace_symbols().await;
  assert!(a.is_ok());

  use owo_colors::{colors::*, OwoColorize};
  for workspace in lsi.workspaces.lock().await.iter() {
    workspace.all_symbols_weak().iter().map(|s| s.upgrade().unwrap()).for_each(|s| {
      println!(
        "symbol: {:#?}\nname: {}\nrange:{:#?}\nwsp: {}\nfp::{}\n{}\n{}",
        s.kind,
        s.name,
        s.range,
        Url::from_file_path(s.workspace_path.clone().canonicalize().unwrap()).unwrap(),
        s.file_path.to_str().unwrap(),
        &s.get_source().unwrap().fg::<Blue>(),
        &s.get_selection().unwrap().fg::<Green>()
      );
    });
    println!("{} workspace symbols found in {} files", workspace.count_symbols(), workspace.files.len());
  }

  let capabilities = lsi.server_capabilities().await;
  assert!(capabilities.is_ok());
  // println!("Capabilities: {:#?}", capabilities.unwrap());
  tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
  panic!();
  /*
  let _c_client = lsi.initialize_client("c", "clangd", None, &[], false).unwrap().unwrap();
  let _cpp_client = lsi.initialize_client("cpp", "clangd", None, &[], false).unwrap().unwrap();
  let _python_client = lsi.initialize_client("python", "jedi", None, root_dirs.as_slice(), false).unwrap().unwrap();
  let _typescript_client =
    lsi.initialize_client("typescript", "typescript-language-server", None, &[], false).unwrap().unwrap();
  let rust_client = lsi.initialize_client("rust", "rust-analyzer", None, root_dirs.as_slice(), false).unwrap().unwrap();

  while !lsi.language_servers.iter_clients().all(|client| {
    println!("Waiting for all clients to be initialized {} {}", client.name(), client.id());
    client.is_initialized()
  }) {
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  }

  println!("All clients initialized");

  println!("Rust client capabilities: {:#?}", rust_client);
  //panic!();

  lsi.language_servers.iter_clients().for_each(|client| {
    println!(
      "{}:
      workspace_symbol_provider: {:?}
      document_symbol_provider: {:?}
      document_link_provider: {:#?}
      references_provider: {:#?}\n",
      client.name(),
      client.capabilities().workspace_symbol_provider,
      client.capabilities().document_symbol_provider,
      client.capabilities().document_link_provider,
      client.capabilities().references_provider
    );
  });

  // println!("{:#?}", rust_client);
  // let timeout = Duration::from_secs(30);
  // tokio::time::timeout(timeout, async {
  let ids = vec![rust_client.id()];
  println!("begin rust-analyzer tests");
  let workspace_symbols = lsi.query_workspace_symbols("main", &ids).await.unwrap();

  println!("Workspace symbols: {:#?}", workspace_symbols);
  let main_rs = workspace_symbols.first().unwrap();
  // assert_eq!(workspace_symbols.len(), 1);
  // assert_eq!(main_rs.name, "main");

  if let OneOf::Left(location) = &main_rs.location {
    let document_symbols = lsi.query_document_symbols(&location.uri, &ids).await.unwrap();
    println!("{:#?}", document_symbols);
    println!("{:#?}", &location.uri);
  }

  println!("{:#?}", test_workspace_path);
  // walk test_workspace_path, and collect all files
  let files = walkdir::WalkDir::new(&test_workspace_path)
    .into_iter()
    .filter_map(|e| e.ok())
    .filter(|e| e.path().is_file())
    .filter(|e| e.path().extension().unwrap_or_default() == "rs")
    .flat_map(|e| e.path().canonicalize())
    .collect::<Vec<PathBuf>>();

  let mut symbols = Vec::new();
  let mut source_symbols = Vec::new();
  for file in files.iter() {
    let uri = Url::from_file_path(file).unwrap();
    assert!(file.exists());
    println!("uri: {:#?}", uri);
    let document_symbols = lsi.query_document_symbols(&uri, &ids).await.unwrap();
    document_symbols.iter().for_each(|s| {
      source_symbols.push(SourceSymbol::from_document_symbol(s, &uri, None));
    });
    symbols.extend(document_symbols);
  }
  println!("{:#?}", symbols);
  assert!(symbols.len() > 1);

  // let newfunc = symbols.iter().find(|s| s.name == "new").unwrap();
  // println!("{:#?}", newfunc);

  let wds_res = lsi.get_workspace_document_symbols(rust_client.id()).await?;

  println!("{:#?}", wds_res);
  assert!(source_symbols.len() > 1);
  source_symbols.iter().for_each(|s| {
    println!("symbols: {}", s);
  });

  assert!(wds_res.len() > 1);
  panic!();
  // })
  // .await
  // .unwrap();
  // let timeout_res = tokio::time::timeout(timeout, async { lsi.wait_for_progress_tokens_completion().await })
  //   .map_err(|f| anyhow::anyhow!("Timeout error: {:#?}", f))
  //   .and_then(|res| {
  //     assert!(res.is_ok());
  //     async {
  //       let rust_workspace_symbols_response = rust_client.workspace_symbols("main".to_string());
  //       if let Some(symbols) = rust_workspace_symbols_response {
  //         let symbols = symbols.await.unwrap();
  //         let symbols = from_value::<Vec<WorkspaceSymbol>>(symbols).unwrap();
  //         for symbol in symbols.iter().filter(|s| s.kind == SymbolKind::FUNCTION) {
  //           println!("Rust Workspace symbol: {:#?}", symbol);
  //         }
  //         // println!("Rust Workspace symbols: {:#?}", symbols);
  //         println!("Rust Workspace symbol count: {:#?}", symbols.len());
  //         anyhow::Ok(())
  //       } else {
  //         Err(anyhow::anyhow!("Rust workspace symbols response is None"))
  //       }
  //     }
  //   })
  //   .and_then(|res| async {
  //     assert!(res.is_ok());
  //
  //     Ok(())
  //   });

  // print out a list of files intest_workspace_path
  // let mut files = vec![];
  // for entry in std::fs::read_dir(&test_workspace_path)? {
  //   let entry = entry?;
  //   let path = entry.path();
  //   if path.is_file() {
  //     files.push(path);
  //   }
  // }
  // println!("Files in test_workspace_path: {:#?}", files);
  // panic!();
  // rust_client.did_change_workspace(vec![src], vec![]).await?;

  // println!("Workspace folders: {:#?}", workspace_folders);

  // tokio::time::sleep(std::time::Duration::from_millis(3000)).await;

      */
  Ok(())
}
