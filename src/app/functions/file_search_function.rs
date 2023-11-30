use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use serde_derive::{Deserialize, Serialize};

use crate::app::functions::types::{FunctionCall, FunctionProperties};
use crate::app::session_config::SessionConfig;
use crate::trace_dbg;

use super::tool_call::ToolCallTrait;
use super::types::FunctionParameters;
use super::{argument_validation::count_tokens, argument_validation::get_accessible_file_paths, ToolCallError};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileSearchFunction {
  pub name: String,
  pub description: String,
  pub required_properties: Vec<FunctionProperties>,
  pub optional_properties: Vec<FunctionProperties>,
}

impl ToolCallTrait for FileSearchFunction {
  fn init() -> Self {
    FileSearchFunction {
        name: "file_search".to_string(),
        description: "search accessible file paths. file_search without arguments returns all accessible file paths. results include file line count".to_string(),
        required_properties: vec![],
        optional_properties: vec![
            FunctionProperties {
                name:  "search_term".to_string(),
                required: true,
                property_type: "string".to_string(),
                description: Some( "fuzzy search for files by name or path. search results contain a match score and line count.".to_string()),
                enum_values: None,
            }
        ]
    }
  }

  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Result<Option<String>, ToolCallError> {
    if let Some(v) = function_args.get("path") {
      if let Some(pathstr) = v.as_str() {
        let accesible_paths = get_accessible_file_paths(session_config.list_file_paths.clone(), None);
        if !accesible_paths.contains_key(Path::new(pathstr).to_str().unwrap()) {
          return Err(ToolCallError::new(format!("File path is not accessible: {:?}", pathstr).as_str()));
        } else {
          trace_dbg!("path: {:?} exists", pathstr);
        }
      }
    }
    let search_term: Option<&str> = function_args.get("search_term").and_then(|s| s.as_str());

    file_search(session_config.function_result_max_tokens, session_config.list_file_paths.clone(), search_term)
  }

  fn function_definition(&self) -> FunctionCall {
    let mut properties: HashMap<String, FunctionProperties> = HashMap::new();

    self.required_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.optional_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    FunctionCall {
      name: self.name.clone(),
      description: Some(self.description.clone()),
      parameters: Some(FunctionParameters {
        param_type: "object".to_string(),
        required: self.required_properties.clone().into_iter().map(|p| p.name).collect(),
        properties,
      }),
    }
  }
}

fn count_lines_and_format_search_results(
  path: &str,
  column_width: usize,
  result_score: Option<&f32>,
) -> Option<String> {
  if !Path::new(path).is_file() {
    return None;
  }
  match File::open(path) {
    Ok(file) => {
      let mut reader = BufReader::new(file);
      let mut buf = String::new();
      match reader.read_to_string(&mut buf) {
        Ok(_) => {
          let linecount = buf.lines().count();
          // format line that is below, but truncates s.1 to 2 decimal places
          match result_score {
            Some(score) => Some(format!("{:column_width$}\t{:<15.2}\t{} lines", path, score, linecount)),
            None => Some(format!("{:column_width$}\t{} lines", path, linecount)),
          }
        },
        Err(e) => Some(format!("error reading file path {} error: {}", path, e)),
      }
    },
    Err(e) => Some(format!("error opening file path {} error: {}", path, e)),
  }
}

fn get_column_width(strings: Vec<&str>) -> usize {
  strings.iter().map(|s| s.len()).max().unwrap_or(0) + 2
}

pub fn file_search(
  reply_max_tokens: usize,
  list_file_paths: Vec<PathBuf>,
  search_term: Option<&str>,
) -> Result<Option<String>, ToolCallError> {
  let paths = get_accessible_file_paths(list_file_paths, None);
  let accessible_paths = paths.keys().map(|path| path.as_str()).collect::<Vec<&str>>();
  // find the length of the longest string in accessible_paths
  let search_results = if let Some(search) = search_term {
    let fuzzy_search_result = rust_fuzzy_search::fuzzy_search_sorted(search, &accessible_paths);
    let column_width = get_column_width(fuzzy_search_result.iter().map(|(s, _)| *s).collect());
    let fuzzy_search_result = fuzzy_search_result
      .iter()
      .filter(|(_, result_score)| result_score > &0.15)
      .filter_map(|(path, result_score)| count_lines_and_format_search_results(path, column_width, Some(result_score)))
      .collect::<Vec<String>>();
    if fuzzy_search_result.is_empty() {
      return Ok(Some("no files matching search term found".to_string()));
    } else {
      fuzzy_search_result.join("\n")
    }
  } else if accessible_paths.is_empty() {
    return Ok(Some("no files are accessible. User must add files to the search path configuration".to_string()));
  } else {
    let column_width = get_column_width(accessible_paths.clone());
    accessible_paths
      .iter()
      .filter_map(|s| count_lines_and_format_search_results(s, column_width, None))
      .collect::<Vec<String>>()
      .join("\n")
  };
  let token_count = count_tokens(&search_results);
  if token_count > reply_max_tokens {
    return Ok(Some(format!("Function Token limit exceeded: {} tokens.", token_count)));
  }
  Ok(Some(search_results))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs::File;
  use std::io::Write;
  use tempfile::tempdir;

  // Helper function to create a file with some content
  fn create_file_with_content(dir: &tempfile::TempDir, file_name: &str, content: &str) -> PathBuf {
    let file_path = dir.path().join(file_name);
    let mut file = File::create(&file_path).expect("Failed to create file.");
    writeln!(file, "{}", content).expect("Failed to write to file.");
    file_path
  }

  #[test]
  fn test_file_search_with_matching_term() {
    let dir = tempdir().expect("Failed to create temp dir.");
    let file_path = create_file_with_content(&dir, "test.txt", "This is a test file containing Rust.");

    let result = file_search(100, vec![file_path], Some("Rust"));

    assert!(result.is_ok());
    let search_results = result.unwrap();
    assert!(search_results.is_some());
    // The exact result depends on the mock implementation and what it returns
    assert!(search_results.unwrap().contains("test.txt"));
  }

  #[test]
  fn test_file_search_without_search_term() {
    let dir = tempdir().expect("Failed to create temp dir.");
    let file_path = create_file_with_content(&dir, "test.txt", "This is a test file.");

    let result = file_search(100, vec![file_path], None);

    assert!(result.is_ok());
    let search_results = result.unwrap();
    assert!(search_results.is_some());
    // The exact result depends on the mock implementation and what it returns
    assert!(search_results.unwrap().contains("test.txt"));
  }

  #[test]
  fn test_file_search_with_no_matching_term() {
    let dir = tempdir().expect("Failed to create temp dir.");
    let file_path = create_file_with_content(&dir, "test.txt", "This is a test file.");

    let result = file_search(100, vec![file_path], Some("Nonexistent"));

    assert!(result.is_ok());
    let search_results = result.unwrap();
    assert!(search_results.is_some());
    assert_eq!(search_results.unwrap(), "no files matching search term found");
  }

  #[test]
  fn test_file_search_with_no_accessible_files() {
    let result = file_search(100, vec![], None);

    assert!(result.is_ok());
    let search_results = result.unwrap();
    assert!(search_results.is_some());
    assert_eq!(
      search_results.unwrap(),
      "no files are accessible. User must add files to the search path configuration"
    );
  }

  #[test]
  fn test_file_search_with_token_limit_exceeded() {
    let dir = tempdir().expect("Failed to create temp dir.");
    let file_path = create_file_with_content(&dir, "test.txt", "This is a test file with a lot of content...");

    let result = file_search(10, vec![file_path], None); // Set a low token limit to trigger the limit

    assert!(result.is_ok());
    let search_results = result.unwrap();
    assert!(search_results.is_some());
    assert!(search_results.unwrap().contains("Function Token limit exceeded"));
  }
}
