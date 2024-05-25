mod test {
  mod helpers;
  use anyhow::Context;
  use helix_lsp::lsp::{self, SymbolKind};
  use helix_view::editor::LspConfig;
  use std::path::{Path, PathBuf};
  use tempfile::tempdir;

  use self::helpers::*;
  use sazid::app::{
    errors::SazidError,
    lsi::{query::LsiQuery, symbol_types::SerializableSourceSymbol},
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

    //let workspace_path =
    //  PathBuf::from("/Users/tenkai/Development/gpt/sazid/sazid-term/tests/test_assets/svd_to_csv");
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

    /*
    query for test function symbol, verify its contents
    */
    let query = LsiQuery {
      session_id: app.get_session_id(),
      name_regex: Some("test_function".to_string()),
      //file_path_regex: Some("src/main.rs".to_string()),
      workspace_root: workspace_path.clone(),
      include_source: true,
      test_query: true,
      ..Default::default()
    };
    app
      .send_language_server_event(sazid::action::LsiAction::QueryWorkspaceSymbols(query.clone()))?;
    run_event_loop_until_idle(&mut app).await;

    match &app.get_session().test_tool_call_response {
      Some((lsi_query, content)) => {
        let symbol = serde_json::from_str::<Vec<SerializableSourceSymbol>>(content)?;
        assert_eq!(query, *lsi_query);
        assert_eq!(symbol.first().unwrap().name, "test_function".to_string());
        assert_eq!(symbol.first().unwrap().detail, Some("fn(text: String) -> bool".to_string()));
        assert_eq!(symbol.first().unwrap().kind, SymbolKind::FUNCTION);
        assert_eq!(symbol.first().unwrap().tags, Some(vec![]));
        assert_eq!(symbol.first().unwrap().range.start, lsp::Position { line: 82, character: 0 });
        assert_eq!(symbol.first().unwrap().range.end, lsp::Position { line: 85, character: 1 });
        assert_eq!(symbol.first().unwrap().workspace_path, workspace_path.clone());
        assert_eq!(symbol.first().unwrap().file_path, PathBuf::from("src/main.rs"));
        assert_eq!(
          symbol.first().unwrap().source_code,
          Some(
            "fn test_function(text: String) -> bool {\n  println!(\"{}\", text);\n  true\n}".into()
          )
        );
      },
      _ => {
        panic!("Expected a response from the language server interface");
      },
    }

    /*
    query for the main function symbol, verify its contents
    */
    let query = LsiQuery {
      session_id: app.get_session_id(),
      name_regex: Some("main".to_string()),
      //file_path_regex: Some("src/main.rs".to_string()),
      workspace_root: workspace_path.clone(),
      include_source: true,
      test_query: true,
      ..Default::default()
    };
    app
      .send_language_server_event(sazid::action::LsiAction::QueryWorkspaceSymbols(query.clone()))?;
    run_event_loop_until_idle(&mut app).await;

    let symbol_id = match &app.get_session().test_tool_call_response {
      Some((lsi_query, content)) => {
        let symbol = serde_json::from_str::<Vec<SerializableSourceSymbol>>(content)?;
        assert_eq!(query, *lsi_query);
        assert_eq!(symbol.first().unwrap().name, "main".to_string());
        assert_eq!(symbol.first().unwrap().detail, Some("fn()".to_string()));
        assert_eq!(symbol.first().unwrap().kind, SymbolKind::FUNCTION);
        assert_eq!(symbol.first().unwrap().tags, Some(vec![]));
        assert_eq!(symbol.first().unwrap().range.start, lsp::Position { line: 18, character: 0 });
        assert_eq!(symbol.first().unwrap().range.end, lsp::Position { line: 58, character: 1 });
        assert_eq!(symbol.first().unwrap().workspace_path, workspace_path.clone());
        assert_eq!(symbol.first().unwrap().file_path, PathBuf::from("src/main.rs"));
        assert_eq!(
                  symbol.first().unwrap().source_code,
        Some("fn main() {\n  let cli = Args::parse();\n  let svd_path = cli.svd_path;\n  let out_path = match cli.output {\n    Some(path) => PathBuf::from(path),\n    None => {\n      let stem = svd_path.file_stem().unwrap().to_str().unwrap();\n      let new_name = format!(\"./{}.csv\", stem);\n      PathBuf::from(new_name)\n    },\n  };\n\n  // Load SVD file\n  let mut file = File::open(svd_path.clone()).expect(\"Could not open SVD file\");\n  let mut contents = String::new();\n  file.read_to_string(&mut contents).expect(\"Could not read SVD file\");\n\n  // Parse SVD file\n  let mut parser_config = svd_parser::Config::default();\n  parser_config.validate_level = ValidateLevel::Weak;\n  parser_config.ignore_enums(true);\n  parser_config.expand(true);\n  parser_config.expand_properties(true);\n\n  let mut device =\n    svd_parser::parse_with_config(&contents, &parser_config).expect(\"Error parsing SVD XML file\");\n\n  // Create a CSV writer\n  let mut wtr = Writer::from_path(out_path.clone()).expect(\"Could not create CSV file\");\n\n  // Iterate over peripherals\n  write_peripheral_to_csv(&mut wtr, device).expect(\"Could not write peripheral details to CSV\");\n\n  wtr.flush().expect(\"Failed to flush CSV writer\");\n\n  println!(\n    \"The SVD file '{}' has been successfully processed into '{}'\",\n    svd_path.display(),\n    out_path.display()\n  );\n}".into())
                );
        symbol.first().unwrap().hash
      },
      _ => {
        panic!("Expected a response from the language server interface");
      },
    };

    /*
    Test to see if querying for the resulting symbol ID returns the same symbol as the previous function
    */
    let query = LsiQuery {
      session_id: app.get_session_id(),
      symbol_id: Some(symbol_id.into()),
      //name_regex: Some("main".to_string()),
      //file_path_regex: Some("src/main.rs".to_string()),
      workspace_root: workspace_path.clone(),
      include_source: true,
      test_query: true,
      ..Default::default()
    };
    app
      .send_language_server_event(sazid::action::LsiAction::QueryWorkspaceSymbols(query.clone()))?;
    run_event_loop_until_idle(&mut app).await;

    match &app.get_session().test_tool_call_response {
      Some((lsi_query, content)) => {
        let symbol = serde_json::from_str::<Vec<SerializableSourceSymbol>>(content)?;
        assert_eq!(query, *lsi_query);
        assert_eq!(symbol.first().unwrap().name, "main".to_string());
        assert_eq!(symbol.first().unwrap().detail, Some("fn()".to_string()));
        assert_eq!(symbol.first().unwrap().kind, SymbolKind::FUNCTION);
        assert_eq!(symbol.first().unwrap().tags, Some(vec![]));
        assert_eq!(symbol.first().unwrap().range.start, lsp::Position { line: 18, character: 0 });
        assert_eq!(symbol.first().unwrap().range.end, lsp::Position { line: 58, character: 1 });
        assert_eq!(symbol.first().unwrap().workspace_path, workspace_path.clone());
        assert_eq!(symbol.first().unwrap().file_path, PathBuf::from("src/main.rs"));
        assert_eq!(
                  symbol.first().unwrap().source_code,
        Some("fn main() {\n  let cli = Args::parse();\n  let svd_path = cli.svd_path;\n  let out_path = match cli.output {\n    Some(path) => PathBuf::from(path),\n    None => {\n      let stem = svd_path.file_stem().unwrap().to_str().unwrap();\n      let new_name = format!(\"./{}.csv\", stem);\n      PathBuf::from(new_name)\n    },\n  };\n\n  // Load SVD file\n  let mut file = File::open(svd_path.clone()).expect(\"Could not open SVD file\");\n  let mut contents = String::new();\n  file.read_to_string(&mut contents).expect(\"Could not read SVD file\");\n\n  // Parse SVD file\n  let mut parser_config = svd_parser::Config::default();\n  parser_config.validate_level = ValidateLevel::Weak;\n  parser_config.ignore_enums(true);\n  parser_config.expand(true);\n  parser_config.expand_properties(true);\n\n  let mut device =\n    svd_parser::parse_with_config(&contents, &parser_config).expect(\"Error parsing SVD XML file\");\n\n  // Create a CSV writer\n  let mut wtr = Writer::from_path(out_path.clone()).expect(\"Could not create CSV file\");\n\n  // Iterate over peripherals\n  write_peripheral_to_csv(&mut wtr, device).expect(\"Could not write peripheral details to CSV\");\n\n  wtr.flush().expect(\"Failed to flush CSV writer\");\n\n  println!(\n    \"The SVD file '{}' has been successfully processed into '{}'\",\n    svd_path.display(),\n    out_path.display()\n  );\n}".into())
                );
        symbol.first().unwrap().hash
      },
      _ => {
        panic!("Expected a response from the language server interface");
      },
    };

    let code_snippet = "fn main() {\n  println!(\"Hello, world!\");\n}".to_string();
    app.send_language_server_event(sazid::action::LsiAction::ReplaceSymbolText(
      code_snippet.clone(),
      query.clone(),
    ))?;
    run_event_loop_until_idle(&mut app).await;

    //println!("DEBUG:::: ----\n\n{:#?}", app.get_session().test_tool_call_response);
    app
      .send_language_server_event(sazid::action::LsiAction::QueryWorkspaceSymbols(query.clone()))?;

    run_event_loop_until_idle(&mut app).await;

    // querying for the same symbol id should result in no symbols found
    match &app.get_session().test_tool_call_response {
      Some((lsi_query, content)) => {
        assert_eq!(content, "no symbols found");
        assert_eq!(query, *lsi_query);
      },
      _ => {
        panic!("Expected a response from the language server interface");
      },
    };

    let query = LsiQuery {
      session_id: app.get_session_id(),
      name_regex: Some("main".to_string()),
      workspace_root: workspace_path.clone(),
      include_source: true,
      test_query: true,
      ..Default::default()
    };
    app
      .send_language_server_event(sazid::action::LsiAction::QueryWorkspaceSymbols(query.clone()))?;

    run_event_loop_until_idle(&mut app).await;
    match &app.get_session().test_tool_call_response {
      Some((lsi_query, content)) => {
        // read the contents of the file at workspace_path joined with file_path
        let file = std::fs::read_to_string(workspace_path.join("src/main.rs"))?;
        let debugprnt = format!("DEBUG:::: ----\n\n{:#?}", file);
        println!("{}", debugprnt);
        let symbol = serde_json::from_str::<Vec<SerializableSourceSymbol>>(content)
          .expect("failed to parse symbol");
        assert_eq!(query, *lsi_query);
        assert_eq!(symbol.first().unwrap().name, "main".to_string());
        assert_eq!(symbol.first().unwrap().detail, Some("fn()".to_string()));
        assert_eq!(symbol.first().unwrap().kind, SymbolKind::FUNCTION);
        assert_eq!(symbol.first().unwrap().tags, Some(vec![]));
        assert_eq!(symbol.first().unwrap().range.start, lsp::Position { line: 18, character: 0 });
        assert_eq!(symbol.first().unwrap().range.end, lsp::Position { line: 20, character: 1 });
        assert_eq!(symbol.first().unwrap().workspace_path, workspace_path.clone());
        assert_eq!(symbol.first().unwrap().file_path, PathBuf::from("src/main.rs"));
        assert_eq!(symbol.first().unwrap().source_code, Some(code_snippet));
        symbol.first().unwrap().hash
      },
      _ => {
        panic!("Expected a response from the language server interface");
      },
    };
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
