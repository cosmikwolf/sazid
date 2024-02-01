use std::str::from_utf8;
use std::sync::{Arc, Mutex};

// tests/lsp_client.rs
use helix_core;
use helix_core::config::default_syntax_loader;
use helix_core::syntax::Loader;
use helix_loader;
use helix_loader::grammar::{get_language, load_runtime_file};
use helix_lsp::{self, Registry};
use lsp_types::*;
use ntest::timeout;
// The actual test function

pub fn test_lang_config() -> helix_core::syntax::Configuration {
  let default_config = include_bytes!("./assets/languages_sazid.toml");
  toml::from_str::<toml::Value>(from_utf8(default_config).unwrap())
    .expect("Could not parse built-in languages.toml to valid toml")
    .try_into()
}

#[tokio::test]
// #[timeout(60000)]
async fn test_rust_analyzer_connection() -> anyhow::Result<()> {
  // let loader = Loader::new(Configuration { language: vec![], language_server: HashMap::new() });
  // let language = get_language("rust").unwrap();

  let config = default_syntax_loader();
  let config = test_lang_config();
  let loader = Loader::new(config);

  // for language in syntax_loader.language {
  //   println!("Adding language: {}", language.language_id);
  // }

  // let language_config = syntax_loader.iter().find(|x| x.language_id == "rust").unwrap();
  //
  // let test_project_path = std::env::current_dir()?.join("tests/assets/testproject");
  // let workspace_folders = Some(vec![WorkspaceFolder {
  //   uri: url::Url::from_directory_path(test_project_path.clone()).unwrap(),
  //   name: "testproject".to_string(),
  // }]);
  //
  // let root_uri = Some(url::Url::from_directory_path(test_project_path).unwrap());

  let registry = Registry::new(Arc::new(loader));
  println!("registry: {:#?}", registry);
  panic!();
  // Create an LspClientStdio instance
  Ok(())
}
