#[cfg(test)]
mod tests {
  use std::{collections::HashMap, path::PathBuf};

  use clap::Parser;
  use sazid::app::{
    model_tools::argument_validation::{
      clap_args_to_json, validate_and_extract_paths_from_argument,
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
    let actual_json: serde_json::Value =
      serde_json::from_str(&clap_args_to_json::<TestArgs>()).unwrap();

    assert_eq!(expected_json, actual_json);
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
    // Set the base directory for file search as the temporary directory
    let temp_dir = tempfile::tempdir().expect("Failed to create a temporary directory");
    println!("{:?}", temp_dir.path());
    let file1 = temp_dir.path().join("file1.rs");
    let file2 = temp_dir.path().join("file2.rs");
    println!("{:?}", file1);
    std::fs::File::create(&file1).expect("Failed to create temporary file1");
    std::fs::File::create(&file2).expect("Failed to create temporary file2");

    let mut function_args = HashMap::new();
    let valid_paths = format!("{},{}", file1.to_str().unwrap(), file2.to_str().unwrap());
    function_args.insert("paths".to_string(), serde_json::Value::String(valid_paths.to_string()));

    let session_config =
      SessionConfig { accessible_paths: vec![temp_dir.path().to_path_buf()], ..Default::default() };
    let result = validate_and_extract_paths_from_argument(
      &function_args,
      session_config,
      true,
      Some(temp_dir.path().to_path_buf()),
    );
    println!("{:?}", result);
    temp_dir.close().expect("Failed to close the temporary directory");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some(vec![file1, file2]));
  }

  #[test]
  fn test_validate_and_extract_paths_from_argument_invalid_paths() {
    let invalid_paths = "./nonexistent/path/to/file1.rs,./nonexistent/path/to/file2.rs";
    let mut function_args = HashMap::new();
    function_args.insert("paths".to_string(), serde_json::Value::String(invalid_paths.to_string()));

    let session_config =
      SessionConfig { accessible_paths: vec![PathBuf::from(".")], ..Default::default() };

    let result =
      validate_and_extract_paths_from_argument(&function_args, session_config, true, None);
    assert!(result.is_err());
  }
  #[test]
  fn test_validate_and_extract_paths_with_invalid_paths() {
    let mut function_args = HashMap::new();
    let session_config = SessionConfig::default();
    function_args.insert(
      "paths".to_string(),
      serde_json::Value::String("./path/to/invalid_file1,./path/to/invalid_file2".to_string()),
    );

    let result =
      validate_and_extract_paths_from_argument(&function_args, session_config, true, None);

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

    let result =
      validate_and_extract_paths_from_argument(&function_args, session_config, true, None);

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.to_string(), "paths argument is required.");
  }

  #[test]
  fn test_validate_and_extract_paths_with_optional_missing_argument() {
    let function_args = HashMap::new();
    let session_config = SessionConfig::default();

    let result =
      validate_and_extract_paths_from_argument(&function_args, session_config, false, None);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
  }
}
