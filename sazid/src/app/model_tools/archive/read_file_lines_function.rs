use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::pin::Pin;

use crate::trace_dbg;
use futures_util::Future;
use serde::{Deserialize, Serialize};

use super::argument_validation::{count_tokens, get_accessible_file_paths};
use super::errors::ToolCallError;
use super::tool_call::{ToolCallParams, ToolCallTrait};
use super::types::{FunctionProperty, PropertyType};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ReadFileLinesFunction {
  pub name: String,
  pub description: String,
  pub properties: Vec<FunctionProperty>,
}

impl ToolCallTrait for ReadFileLinesFunction {
  fn name(&self) -> &str {
    &self.name
  }

  fn init() -> Self {
    ReadFileLinesFunction {
      name: "read_file".to_string(),
      description: "read lines from an accesible file path, from optional 1 indexed start_line to end_line".to_string(),
      properties: vec![
          FunctionProperty {
        name: "path".to_string(),
        required: true,
          property_type: PropertyType::String,
        description: Some("path to file".to_string()),
        enum_values: None,
          },
        FunctionProperty {
          name: "start_line".to_string(),
          required: false,
          property_type: PropertyType::String,
          description: Some("first line to read, default: 1".to_string()),
          enum_values: None,
        },
        FunctionProperty {
          name: "end_line".to_string(),
          required: false,
          property_type: PropertyType::String,
          description: Some("last line to read, default: EOF".to_string()),
          enum_values: None,
        },
      ],
    }
  }

  fn call(
    &self,
    params: ToolCallParams,
  ) -> Pin<
    Box<
      dyn Future<Output = Result<Option<String>, ToolCallError>>
        + Send
        + 'static,
    >,
  > {
    Box::pin(async move {
      let start_line: Option<usize> = params
        .function_args
        .get("start_line")
        .and_then(|s| s.as_u64().map(|u| u as usize));
      let end_line: Option<usize> = params
        .function_args
        .get("end_line")
        .and_then(|s| s.as_u64().map(|u| u as usize));
      if let Some(v) = params.function_args.get("path") {
        if let Some(file) = v.as_str() {
          let accesible_paths = get_accessible_file_paths(
            params.session_config.accessible_paths.clone(),
            None,
          );
          if !accesible_paths.contains_key(Path::new(file).to_str().unwrap()) {
            Err(ToolCallError::new(
            format!("File path is not accessible: {:?}. Suggest using file_search command", file).as_str(),
          ))
          } else {
            trace_dbg!("path: {:?} exists", file);
            read_file_lines(
              file,
              start_line,
              end_line,
              params.session_config.function_result_max_tokens,
              params.session_config.accessible_paths.clone(),
            )
          }
        } else {
          Err(ToolCallError::new("path argument must be a string"))
        }
      } else {
        Err(ToolCallError::new("path argument is required"))
      }
    })
  }

  fn parameters(&self) -> Vec<FunctionProperty> {
    self.properties.clone()
  }

  fn description(&self) -> String {
    self.description.clone()
  }
}

pub fn read_file_lines(
  file: &str,
  start_line: Option<usize>,
  end_line: Option<usize>,
  reply_max_tokens: usize,
  list_file_paths: Vec<PathBuf>,
) -> Result<Option<String>, ToolCallError> {
  // trace_dbg!("list_file_paths: {:?}", list_file_paths);
  // trace_dbg!("file: {:?} {:#?}", get_accessible_file_paths(list_file_paths.clone()).get(file), file);
  if let Some(file_path) =
    get_accessible_file_paths(list_file_paths, None).get(file)
  {
    let file_contents = match read_lines(file_path) {
      Ok(contents) => contents,
      Err(error) => {
        return Err(ToolCallError::new(
          format!("Error reading file: {}\nare you sure a file exists at the provided path?", error).as_str(),
        ));
      },
    };

    // individually validate start_line and end_line and make sure that if they are Some(value) that they are within the respective bounds of the file

    if let Some(start_line) = start_line {
      if start_line > file_contents.len() {
        return Err(ToolCallError::new("Invalid start line number."));
      }
    }

    if let Some(end_line) = end_line {
      if end_line > file_contents.len() {
        return Err(ToolCallError::new("Invalid end line number."));
      }
    }
    let selected_lines: Vec<String> = file_contents
      [start_line.unwrap_or(0)..end_line.unwrap_or(file_contents.len())]
      .to_vec();
    let contents = selected_lines.join("\n");

    let token_count = count_tokens(&contents);
    if token_count > reply_max_tokens {
      return Ok(Some(format!(
        "Function Token limit exceeded: {} tokens.",
        token_count
      )));
    }
    Ok(Some(contents))
  } else {
    Err(ToolCallError::new(
      "File not found or not accessible.\nare you sure a file exists at the path you are accessing?",
    ))
  }
}

fn read_lines(file_path: &Path) -> Result<Vec<String>, std::io::Error> {
  let file = File::open(file_path)?;
  let reader = BufReader::new(file);
  reader.lines().collect()
}

#[cfg(test)]
mod tests {
  use std::fs;

  use super::*;
  use tempfile::tempdir;

  #[test]
  fn test_read_file_within_token_limit() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("5.txt");
    fs::write(&file_path, "1\n2\n3\n4\n5").unwrap();
    let list_file_paths = vec![file_path];

    let result =
      read_file_lines("5.txt", Some(1), Some(3), 10, list_file_paths);

    assert!(result.is_ok());
    let output = result.unwrap().unwrap();
    assert!(output.contains("1\n2\n3"));
    assert!(output.ends_with('3'));
  }

  #[test]
  fn test_read_file_exceeding_token_limit() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("5.txt");
    fs::write(&file_path, "1\n2\n3\n4\n5").unwrap();
    let list_file_paths = vec![file_path];

    let result = read_file_lines("5.txt", None, None, 3, list_file_paths);

    assert!(result.is_ok());
    let output = result.unwrap().unwrap();
    assert!(output.contains("Function Token limit exceeded:"));
  }

  #[test]
  fn test_read_file_lines_with_invalid_start_line() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("5.txt");
    fs::write(&file_path, "1\n2\n3\n4\n5").unwrap();
    let list_file_paths = vec![file_path];

    let result = read_file_lines("5.txt", Some(6), None, 10, list_file_paths);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Invalid start line number.");
  }

  #[test]
  fn test_read_file_lines_with_invalid_end_line() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("5.txt");
    fs::write(&file_path, "1\n2\n3\n4\n5").unwrap();
    let list_file_paths = vec![file_path];

    let result = read_file_lines("5.txt", None, Some(10), 10, list_file_paths);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Invalid end line number.");
  }

  #[test]
  fn test_read_file_lines_file_not_found() {
    let list_file_paths = vec![]; // No files available

    let result =
      read_file_lines("nonexistent.txt", None, None, 10, list_file_paths);

    assert!(result.is_err());
    assert_eq!(
      result.unwrap_err().to_string(),
      "File not found or not accessible.\nare you sure a file exists at the path you are accessing?"
    );
  }
}
