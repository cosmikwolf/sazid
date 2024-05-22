mod test {
  mod helpers;
  use anyhow::Context;
  use helix_view::editor::LspConfig;
  use std::path::{Path, PathBuf};
  use tempfile::tempdir;

  use self::helpers::*;
  use sazid::app::errors::SazidError;
  use sazid_term::{application::Application, args::Args, config::Config};

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
    let args = Args {
      workspace: Some(PathBuf::from("path/to/workspace")),
      language: Some("rust".to_string()),
      ..Default::default()
    };

    helix_loader::initialize_config_file(args.config_file.clone());
    helix_loader::initialize_log_file(args.log_file.clone());
    sazid::utils::initialize_panic_handler().map_err(SazidError::PanicHandlerError).unwrap();

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

    println!("Test workspace path: {:#?}", test_workspace_path);
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
    let workspace_path = setup_test_rust_project("tests/test_assets/svd_to_csv")?;
    std::env::set_current_dir(&workspace_path).unwrap();

    let mut app = setup_app(Some(workspace_path))?;

    app
      .send_chat_tool_event(sazid::action::ChatToolAction::ToolListRequest(app.get_session_id()))?;

    run_event_loop_until_idle(&mut app).await;
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
