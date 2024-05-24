mod test {
  mod helpers;
  use anyhow::Context;
  use helix_lsp::lsp::{self, SymbolKind};
  use helix_view::editor::LspConfig;
  use std::{
    path::{Path, PathBuf},
    sync::Arc,
  };
  use tempfile::tempdir;

  use self::helpers::*;
  use sazid::app::{
    errors::SazidError,
    lsi::{
      query::LsiQuery,
      symbol_types::{SerializableSourceSymbol, SourceSymbol},
    },
    tools::utils::initialize_logging,
  };
  use sazid_term::{application::Application, args::Args, config::Config};

  use tracing_error::ErrorLayer;
  use tracing_subscriber::{
    self, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, Layer,
  };

  const LOG_FILE: &str = "/tmp/sazid.log";

  pub fn initialize_test_logging() -> anyhow::Result<()> {
    let log_path = PathBuf::from(LOG_FILE);
    let log_file = std::fs::File::create(log_path)?;
    let file_subscriber = tracing_subscriber::fmt::layer()
      .with_file(true)
      .with_line_number(true)
      .with_writer(log_file)
      .with_target(false)
      .with_ansi(false)
      .with_filter(tracing_subscriber::filter::EnvFilter::from_default_env());
    tracing_subscriber::registry().with(file_subscriber).with(ErrorLayer::default()).init();
    Ok(())
  }
  fn setup_app(workspace: Option<PathBuf>) -> anyhow::Result<Application> {
    //let mut session_config = SessionConfig::default();
    //let workspace_path = PathBuf::from("path/to/workspace");
    //let language = "rust".to_string();
    //
    //session_config.workspace = Some(WorkspaceParams {
    //  workspace_path,
    //  language,
    //  language_server: "rust-analyzer".to_string(),
    //  doc_path: None,
    //});
    //
    //let lang_loader = helix_core::config::default_lang_loader();
    //
    //let syn_loader = Arc::new(ArcSwap::from_pointee(lang_loader));
    //
    //let (lsi_tx, lsi_rx) = mpsc::unbounded_channel();
    //let language_server_interface_events = UnboundedReceiverStream::new(lsi_rx);
    //let language_server_interface = LanguageServerInterface::new(syn_loader.clone(), lsi_tx);
    //let (session_tx, session_rx) = mpsc::unbounded_channel();
    //let mut session = Session::new(session_tx, Some(session_config));
    //session.set_system_prompt("you are an expert programming assistant");
    let args = Args { workspace, language: Some("rust".to_string()), ..Default::default() };

    helix_loader::initialize_config_file(args.config_file.clone());
    helix_loader::initialize_log_file(args.log_file.clone());
    sazid::utils::initialize_panic_handler().map_err(SazidError::PanicHandlerError).unwrap();

    initialize_test_logging().unwrap();

    let lang_loader = helix_core::config::default_lang_loader();
    let config = test_config();
    Application::new(args, config, lang_loader).context("unable to create new application")
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

  fn setup_test_rust_project(test_workspace_src_path: &str) -> anyhow::Result<PathBuf> {
    let test_src_assets = std::env::current_dir().unwrap().join(test_workspace_src_path);

    // create temp dir for test
    let temp_dir = tempdir()?;
    let test_workspace_path = temp_dir.into_path().join(test_workspace_src_path);

    log::info!("Test workspace path: {:#?}", test_workspace_path);
    // recursively copy test_src_assets into temp_dir
    copy_dir_recursively(&test_src_assets, &test_workspace_path).unwrap();

    assert!(test_workspace_path.exists());

    Ok(test_workspace_path)
  }

  /// Generates a config with defaults more suitable for integration tests
  pub fn test_config() -> Config {
    Config {
      editor: test_editor_config(),
      keys: sazid_term::keymap::default(),
      ..Default::default()
    }
  }

  pub fn test_editor_config() -> helix_view::editor::Config {
    helix_view::editor::Config {
      lsp: LspConfig { enable: false, ..Default::default() },
      ..Default::default()
    }
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn test_read_symbol_source() -> anyhow::Result<()> {
    let workspace_path = setup_test_rust_project("tests/test_assets/svd_to_csv")?.canonicalize()?;
    std::env::set_current_dir(&workspace_path).unwrap();
    let mut app = setup_app(Some(workspace_path.clone()))?;

    //app
    //  .send_chat_tool_event(sazid::action::ChatToolAction::ToolListRequest(app.get_session_id()))?;

    run_event_loop_until_idle(&mut app).await;
    let query = LsiQuery {
      session_id: app.get_session_id(),
      //file_path_regex: Some("src/main.rs".to_string()),
      workspace_root: workspace_path.clone(),
      test_query: true,
      ..Default::default()
    };
    app.send_language_server_event(sazid::action::LsiAction::GetWorkspaceFiles(query.clone()))?;
    run_event_loop_until_idle(&mut app).await;
    assert_eq!(
      app.get_session().test_tool_call_response,
      Some((query.clone(), "[\"src/main.rs\"]".to_string()))
    );

    let query = LsiQuery {
      session_id: app.get_session_id(),
      name_regex: Some("test_function".to_string()),
      //file_path_regex: Some("src/main.rs".to_string()),
      workspace_root: workspace_path.clone(),
      test_query: true,
      ..Default::default()
    };
    app
      .send_language_server_event(sazid::action::LsiAction::QueryWorkspaceSymbols(query.clone()))?;
    run_event_loop_until_idle(&mut app).await;

    match &app.get_session().test_tool_call_response {
      Some((lsi_query, content)) => {
        let symbol = serde_json::from_str::<Vec<SerializableSourceSymbol>>(content)?;
        let main_symbol = SerializableSourceSymbol {
          name: "test_function".to_string(),
          detail: Some("fn(text: String) -> bool".to_string()),
          kind: SymbolKind::FUNCTION,
          tags: Some(vec![]),
          range: lsp::Range {
            start: lsp::Position { line: 82, character: 0 },
            end: lsp::Position { line: 85, character: 1 },
          },
          workspace_path: workspace_path.clone(),
          file_path: PathBuf::from("src/main.rs"),
          hash: [0; 32],
        };
        assert_eq!(query, *lsi_query);
        assert_eq!(symbol.first().unwrap().name, main_symbol.name);
        assert_eq!(symbol.first().unwrap().detail, main_symbol.detail);
        assert_eq!(symbol.first().unwrap().kind, main_symbol.kind);
        assert_eq!(symbol.first().unwrap().tags, main_symbol.tags);
        assert_eq!(symbol.first().unwrap().range, main_symbol.range);
        assert_eq!(symbol.first().unwrap().workspace_path, main_symbol.workspace_path);
        assert_eq!(symbol.first().unwrap().file_path, main_symbol.file_path);
      },
      _ => {
        panic!("Expected a response from the language server interface");
      },
    }

    app.send_language_server_event(sazid::action::LsiAction::ReadSymbolSource(query.clone()))?;
    run_event_loop_until_idle(&mut app).await;

    match &app.get_session().test_tool_call_response {
      Some((lsi_query, content)) => {
        assert_eq!(query, *lsi_query);
        assert_eq!(
          content.as_str(),
          "fn test_function(text: String) -> bool {\n  println!(\"{}\", text);\n  true\n}"
        );
      },
      _ => {
        panic!("Expected a response from the language server interface");
      },
    }

    let query = LsiQuery {
      session_id: app.get_session_id(),
      name_regex: Some("main".to_string()),
      //file_path_regex: Some("src/main.rs".to_string()),
      workspace_root: workspace_path.clone(),
      test_query: true,
      ..Default::default()
    };
    app
      .send_language_server_event(sazid::action::LsiAction::QueryWorkspaceSymbols(query.clone()))?;
    run_event_loop_until_idle(&mut app).await;

    match &app.get_session().test_tool_call_response {
      Some((lsi_query, content)) => {
        let symbol = serde_json::from_str::<Vec<SerializableSourceSymbol>>(content)?;
        let main_symbol = SerializableSourceSymbol {
          name: "main".to_string(),
          detail: Some("fn()".to_string()),
          kind: SymbolKind::FUNCTION,
          tags: Some(vec![]),
          range: lsp::Range {
            start: lsp::Position { line: 18, character: 0 },
            end: lsp::Position { line: 58, character: 1 },
          },
          workspace_path: workspace_path.clone(),
          file_path: PathBuf::from("src/main.rs"),
          hash: [0; 32],
        };
        assert_eq!(query, *lsi_query);
        assert_eq!(symbol.first().unwrap().name, main_symbol.name);
        assert_eq!(symbol.first().unwrap().detail, main_symbol.detail);
        assert_eq!(symbol.first().unwrap().kind, main_symbol.kind);
        assert_eq!(symbol.first().unwrap().tags, main_symbol.tags);
        assert_eq!(symbol.first().unwrap().range, main_symbol.range);
        assert_eq!(symbol.first().unwrap().workspace_path, main_symbol.workspace_path);
        assert_eq!(symbol.first().unwrap().file_path, main_symbol.file_path);
      },
      _ => {
        panic!("Expected a response from the language server interface");
      },
    }

    app.send_language_server_event(sazid::action::LsiAction::ReadSymbolSource(query.clone()))?;
    run_event_loop_until_idle(&mut app).await;

    match &app.get_session().test_tool_call_response {
      Some((lsi_query, content)) => {
        println!("content: {:#?}", content);
        assert_eq!(query, *lsi_query);
        assert_eq!(
          content.as_str(),
          "fn test_function(text: String) -> bool {\n  println!(\"{}\", text);\n true\n }"
        );
      },
      _ => {
        panic!("Expected a response from the language server interface");
      },
    }
    Ok(())
  }

  #[test]
  fn test_cargo_check_with_valid_package() {
    // Setup and call the `cargo_check` function with a valid package name
    // Assert that the result is a success and contains expected data
  }

  #[test]
  fn test_cargo_check_with_invalid_package() {
    // Setup and call the `cargo_check` function with an invalid package name
    // Assert that the result is an error and matches expected error handling behavior
  }
}
