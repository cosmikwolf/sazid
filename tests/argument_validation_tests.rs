#[cfg(test)]
mod tests {
  use std::{collections::HashMap, path::PathBuf};

  use clap::Parser;
  use sazid::app::{
    functions::argument_validation::{
      clap_args_to_json, validate_and_extract_options, validate_and_extract_paths_from_argument,
      validate_and_extract_string_argument,
    },
    session_config::SessionConfig,
  };

  // Define a test struct for clap parsing
  #[derive(Parser, Debug)]
  struct TestArgs {
    #[clap(short = 'a', long = "arg", help = "An example argument")]
    arg: Option<String>,
  }

  #[test]
  fn test_clap_args_to_json() {
    let expected_json: serde_json::Value = serde_json::from_str(
      r#"[
    {"a": "An example argument"}
  ]"#,
    )
    .unwrap();
    let actual_json: serde_json::Value = serde_json::from_str(&clap_args_to_json::<TestArgs>()).unwrap();

    assert_eq!(expected_json, actual_json);
  }

  #[test]
  fn test_validate_and_extract_options_with_required() {
    let function_args = HashMap::new();
    assert!(validate_and_extract_options::<TestArgs>(&function_args, true).is_err());
  }

  #[test]
  fn test_validate_and_extract_options_with_optional() {
    let function_args = HashMap::new();
    assert!(validate_and_extract_options::<TestArgs>(&function_args, false).is_ok());
  }

  #[test]
  fn test_validate_and_extract_string_argument_with_required() {
    let mut function_args = HashMap::new();
    function_args.insert("arg".to_string(), serde_json::Value::String("value".to_string()));
    let result = validate_and_extract_string_argument(&function_args, "arg", true);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some("value".to_string()));
  }

  #[test]
  fn test_validate_and_extract_string_argument_with_optional() {
    let function_args = HashMap::new();
    let result = validate_and_extract_string_argument(&function_args, "arg", false);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
  }

  #[test]
  fn test_validate_and_extract_paths_from_argument_valid_paths() {
    let valid_paths = "./path/to/file1.rs,./path/to/file2.rs";
    let mut function_args = HashMap::new();
    function_args.insert("paths".to_string(), serde_json::Value::String(valid_paths.to_string()));

    let session_config = SessionConfig { list_file_paths: vec![PathBuf::from(".")], ..Default::default() };

    // Use tempdir for file creation to avoid errors related to file paths
    let temp_dir = tempfile::tempdir().unwrap();
    let file1 = temp_dir.path().join("file1.rs");
    let file2 = temp_dir.path().join("file2.rs");

    std::fs::File::create(file1.clone()).unwrap();
    std::fs::File::create(file2.clone()).unwrap();

    let result = validate_and_extract_paths_from_argument(&function_args, session_config, true);
    temp_dir.close().unwrap();

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some(vec![file1, file2]));
  }

  #[test]
  fn test_validate_and_extract_paths_from_argument_invalid_paths() {
    let invalid_paths = "./nonexistent/path/to/file1.rs,./nonexistent/path/to/file2.rs";
    let mut function_args = HashMap::new();
    function_args.insert("paths".to_string(), serde_json::Value::String(invalid_paths.to_string()));

    let session_config = SessionConfig { list_file_paths: vec![PathBuf::from(".")], ..Default::default() };

    let result = validate_and_extract_paths_from_argument(&function_args, session_config, true);
    assert!(result.is_err());
  }

  #[test]
  fn test_validate_and_extract_paths_with_valid_paths() {
    let mut function_args = HashMap::new();
    let session_config = SessionConfig::default();
    function_args.insert("paths".to_string(), serde_json::Value::String("./path/to/file1,./path/to/file2".to_string()));

    let result = validate_and_extract_paths_from_argument(&function_args, session_config, true);

    assert!(result.is_ok());
    let paths = result.unwrap().unwrap();
    assert_eq!(paths, vec![PathBuf::from("./path/to/file1"), PathBuf::from("./path/to/file2")]);
  }

  #[test]
  fn test_validate_and_extract_paths_with_invalid_paths() {
    let mut function_args = HashMap::new();
    let session_config = SessionConfig::default();
    function_args.insert(
      "paths".to_string(),
      serde_json::Value::String("./path/to/invalid_file1,./path/to/invalid_file2".to_string()),
    );

    let result = validate_and_extract_paths_from_argument(&function_args, session_config, true);

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(
      error.to_string(),
      "File path is not accessible: \"./path/to/invalid_file1\". Suggest using file_search command"
    );
  }

  #[test]
  fn test_validate_and_extract_paths_with_missing_required_argument() {
    let function_args = HashMap::new();
    let session_config = SessionConfig::default();

    let result = validate_and_extract_paths_from_argument(&function_args, session_config, true);

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "paths argument is required.");
  }

  #[test]
  fn test_validate_and_extract_paths_with_optional_missing_argument() {
    let function_args = HashMap::new();
    let session_config = SessionConfig::default();

    let result = validate_and_extract_paths_from_argument(&function_args, session_config, false);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
  }
}

