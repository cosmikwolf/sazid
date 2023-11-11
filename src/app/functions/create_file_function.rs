use std::{
  collections::HashMap,
  fs::{self, File},
  io::Write,
  path::Path,
};

use crate::app::session_config::SessionConfig;
use serde_derive::{Deserialize, Serialize};

use super::{
  errors::FunctionCallError,
  function_call::FunctionCall,
  types::{Command, CommandParameters, CommandProperty},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreateFileFunction {
  name: String,
  description: String,
  required_properties: Vec<CommandProperty>,
  optional_properties: Vec<CommandProperty>,
}

impl FunctionCall for CreateFileFunction {
  fn init() -> Self {
    CreateFileFunction {
      name: "create_file".to_string(),
      description: "create a file at path with text".to_string(),
      required_properties: vec![
        CommandProperty {
          name: "path".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some("path to file".to_string()),
          enum_values: None,
        },
        CommandProperty {
          name: "text".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some("text to write to file".to_string()),
          enum_values: None,
        },
      ],
      optional_properties: vec![],
    }
  }

  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    _session_config: SessionConfig,
  ) -> Result<Option<String>, FunctionCallError> {
    let path: Option<&str> = function_args.get("path").and_then(|s| s.as_str());
    let text: Option<&str> = function_args.get("text").and_then(|s| s.as_str());
    if let Some(path) = path {
      if let Some(text) = text {
        create_file(path, text)
      } else {
        Err(FunctionCallError::new("text argument is required"))
      }
    } else {
      Err(FunctionCallError::new("path argument is required"))
    }
  }

  fn command_definition(&self) -> Command {
    let mut properties: HashMap<String, CommandProperty> = HashMap::new();

    self.required_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.optional_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    Command {
      name: self.name.clone(),
      description: Some(self.description.clone()),
      parameters: Some(CommandParameters {
        param_type: "object".to_string(),
        required: self.required_properties.clone().into_iter().map(|p| p.name).collect(),
        properties,
      }),
    }
  }
}

pub fn create_file(path: &str, text: &str) -> Result<Option<String>, FunctionCallError> {
  // Convert the string path to a `Path` object to manipulate file paths.
  let path = Path::new(path);

  // Attempt to get the parent directory of the path.
  if let Some(parent_dir) = path.parent() {
    // Try to create the parent directory (and all necessary parent directories).
    if let Err(e) = fs::create_dir_all(parent_dir) {
      // If there's an error creating the directory, return the error.
      return Ok(Some(format!("error creating directory: {}", e)));
    }
  } else {
    // If there's no parent directory in the path, return an error message.
    return Ok(Some("error obtaining parent directory".to_string()));
  }

  // Proceed to create the file now that the parent directories should exist.
  match File::create(path) {
    Ok(mut file) => match file.write_all(text.as_bytes()) {
      Ok(_) => Ok(Some("file created".to_string())),
      Err(e) => Ok(Some(format!("error writing file: {}", e))),
    },
    Err(e) => Ok(Some(format!("error creating file: {}", e))),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::{Read, Write};
  use tempdir::TempDir;

  // Test creating a file in an existing directory.
  #[test]
  fn test_create_file_in_existing_directory() {
    let tmp_dir = TempDir::new("test_create_file").unwrap();
    let file_path = tmp_dir.path().join("test_file.txt");
    let file_contents = "Test file contents.";

    let result = create_file(file_path.to_str().unwrap(), file_contents);
    assert!(result.is_ok());
    check_file_contents(&file_path, file_contents);
  }

  // Test creating a file in a non-existent directory.
  #[test]
  fn test_create_file_in_nonexistent_directory() {
    let tmp_dir = TempDir::new("test_create_file").unwrap();
    let non_existent_subfolder = tmp_dir.path().join("subfolder");
    let file_path = non_existent_subfolder.join("test_file.txt");
    let file_contents = "Test file contents.";

    let result = create_file(file_path.to_str().unwrap(), file_contents);
    assert!(result.is_ok());
    check_file_contents(&file_path, file_contents);
  }

  // Test creating a file with an invalid file name.
  #[test]
  fn test_create_file_with_invalid_file_name() {
    let tmp_dir = TempDir::new("test_create_file").unwrap();
    let file_path = tmp_dir.path().join("\0"); // Null byte is not allowed in file names.
    let file_contents = "Test file contents.";

    let result = create_file(file_path.to_str().unwrap(), file_contents);
    assert!(result.is_ok());
    assert!(result.unwrap().unwrap().contains("error"));
  }

  // Test creating a file where we don't have the correct permissions.
  #[test]
  #[ignore] // Ignored by default due to system dependent behavior
  fn test_create_file_permission_error() {
    // `permissions_dir` should be a path to a directory with restricted permissions.
    let permissions_dir = "/path/to/restricted/directory";
    let file_path = Path::new(permissions_dir).join("test_file.txt");
    let file_contents = "Test file contents.";

    let result = create_file(file_path.to_str().unwrap(), file_contents);
    assert!(result.is_ok());
    assert!(result.unwrap().unwrap().contains("error"));
  }

  // Test creating a file on a read-only filesystem.
  #[test]
  #[ignore] // Ignored by default due to system dependent behavior
  fn test_create_file_read_only_filesystem() {
    // `read_only_dir` should be a path to a directory on a read-only file system.
    let read_only_dir = "/path/to/read-only/directory";
    let file_path = Path::new(read_only_dir).join("test_file.txt");
    let file_contents = "Test file contents.";

    let result = create_file(file_path.to_str().unwrap(), file_contents);
    assert!(result.is_ok());
    assert!(result.unwrap().unwrap().contains("error"));
  }

  // Test creating a file that already exists.
  #[test]
  fn test_create_file_already_exists() {
    let tmp_dir = TempDir::new("test_create_file").unwrap();
    let file_path = tmp_dir.path().join("test_file.txt");
    let initial_contents = "Initial contents.";
    let new_contents = "New contents replacing initial contents.";

    {
      // Create the initial file.
      let mut file = File::create(&file_path).unwrap();
      file.write_all(initial_contents.as_bytes()).unwrap();
    }

    // Perform the operation to create the file again with different contents.
    let result = create_file(file_path.to_str().unwrap(), new_contents);
    assert!(result.is_ok());
    check_file_contents(&file_path, new_contents);
  }

  // Test handling very large input correctly.
  #[test]
  fn test_create_file_with_large_input() {
    let tmp_dir = TempDir::new("test_create_file").unwrap();
    let file_path = tmp_dir.path().join("large_test_file.txt");
    let file_contents = "a".repeat(10_000_000); // 10 MB of 'a'.

    let result = create_file(file_path.to_str().unwrap(), &file_contents);
    assert!(result.is_ok());
    check_file_contents(&file_path, &file_contents);
  }

  // Helper function to check the contents of a file match the expected text.
  fn check_file_contents(file_path: &Path, expected_text: &str) {
    let mut file = File::open(file_path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    assert_eq!(contents, expected_text);
  }
}
