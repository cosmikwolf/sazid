use grep::{
  printer::StandardBuilder,
  regex::RegexMatcher,
  searcher::{BinaryDetection, SearcherBuilder},
};
use serde_derive::{Deserialize, Serialize};

use std::{collections::HashMap, io::Write};
use std::{
  io::BufWriter,
  path::{Path, PathBuf},
};
use walkdir::WalkDir;

use crate::app::session_config::SessionConfig;

use super::{
  get_accessible_file_paths,
  types::{Command, CommandParameters, CommandProperty},
  FunctionCall, FunctionCallError,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrepFunction {
  pub name: String,
  pub description: String,
  pub required_properties: Vec<CommandProperty>,
  pub optional_properties: Vec<CommandProperty>,
}

impl FunctionCall for GrepFunction {
  fn init() -> Self {
    GrepFunction {
      name: "grep".to_string(),
      description: "an implementation of grep".to_string(),
      required_properties: vec![
        CommandProperty {
          name: "pattern".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some("a regular expression pattern to match against file contents".to_string()),
          enum_values: None,
        },
        CommandProperty {
          name: "paths".to_string(),
          required: true,
          property_type: "array".to_string(),
          description: Some("a list of paths to walk for files which the pattern will be matched against".to_string()),
          enum_values: None,
        },
      ],
      optional_properties: vec![
        // todo: implement multi_line

      // CommandProperty {
      //   name: "multi_line".to_string(),
      //   required: false,
      //   property_type: "bool".to_string(),
      //   description: Some("match pattern over multiple lines. default is false".to_string()),
      //   enum_values: None,
      // }
      ],
    }
  }

  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Result<Option<String>, FunctionCallError> {
    let paths = match function_args.get("paths") {
      Some(paths) => match validate_and_extract_paths_from_argument(paths.clone(), session_config) {
        Ok(paths) => paths,
        Err(err) => return Err(err),
      },
      None => return Err(FunctionCallError::new("paths argument is required")),
    };

    let pattern: Option<&str> = function_args.get("pattern").and_then(|s| s.as_str());

    let _multi_line: Option<bool> = function_args.get("multi_line").and_then(|s| s.as_bool());
    match pattern {
      Some(pattern) => grep(pattern, paths),
      None => Err(FunctionCallError::new("pattern argument is required")),
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

pub fn validate_and_extract_paths_from_argument(
  paths: serde_json::Value,
  session_config: SessionConfig,
) -> Result<Vec<PathBuf>, FunctionCallError> {
  let mut paths_vec: Vec<PathBuf> = Vec::new();
  if let serde_json::Value::Array(paths) = paths {
    for path_value in paths {
      if let Some(pathstr) = path_value.as_str() {
        let accesible_paths = get_accessible_file_paths(session_config.list_file_paths.clone());
        if !accesible_paths.contains_key(Path::new(pathstr).to_str().unwrap()) {
          return Err(FunctionCallError::new(
            format!("File path is not accessible: {:?}. Suggest using file_search command", pathstr).as_str(),
          ));
        } else {
          paths_vec.push(pathstr.into());
        }
      }
    }
  }
  Ok(paths_vec)
}

pub fn grep(pattern: &str, paths: Vec<PathBuf>) -> Result<Option<String>, FunctionCallError> {
  //let mut buffer = Cursor::new(Vec::new());
  let buffer: BufWriter<Vec<u8>> = BufWriter::new(Vec::new());
  let mut error_buffer: BufWriter<Vec<u8>> = BufWriter::new(Vec::new());
  match RegexMatcher::new(pattern) {
    Ok(matcher) => {
      let mut searcher =
        SearcherBuilder::new().binary_detection(BinaryDetection::quit(b'\x00')).line_number(false).build();
      let mut printer = StandardBuilder::new().build_no_color(buffer);

      for path in paths {
        for result in WalkDir::new(path) {
          let dent = match result {
            Ok(dent) => dent,
            Err(err) => {
              error_buffer.write_all(format!("{}\n", err).as_bytes()).unwrap();
              continue;
            },
          };
          if !dent.file_type().is_file() {
            continue;
          }
          let result = searcher.search_path(&matcher, dent.path(), printer.sink_with_path(&matcher, dent.path()));
          if let Err(err) = result {
            error_buffer.write_all(format!("{}: {}", dent.path().display(), err).as_bytes()).unwrap();
          }
        }
      }
      match printer.into_inner().into_inner().into_inner() {
        Ok(matches) => {
          if matches.is_empty() {
            error_buffer.write_all(format!("No matches found for pattern: {}", pattern).as_bytes()).unwrap();
          }

          Ok(Some(
            String::from_utf8(matches).unwrap_or_else(|_| "Error parsing grep output text".to_string())
              + String::from_utf8(error_buffer.into_inner().unwrap())
                .unwrap_or_else(|_| "Error parsing grep output text".to_string())
                .as_str(),
          ))
        },
        Err(err) => {
          error_buffer.write_all(format!("Error parsing grep output text: {:?}", err).as_bytes()).unwrap();
          Ok(Some(String::from_utf8(error_buffer.into_inner().unwrap()).unwrap()))
        },
      }
    },
    Err(err) => Ok(Some(format!("Error parsing grep pattern: {:?}", err))),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;
  use std::fs::File;
  use std::io::Write;
  use std::path::PathBuf;
  use tempfile::tempdir;

  // Helper function to write to a file
  fn write_to_file(file_path: &PathBuf, content: &str) -> Result<(), std::io::Error> {
    let mut file = File::create(file_path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
  }

  #[test]
  fn test_grep_basic_single_line_match() -> Result<(), Box<dyn Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("single_line.txt");
    write_to_file(&file_path, "Hello\nRust\nWorld")?;

    let pattern = "Rust";
    let paths = vec![file_path];
    let result = grep(pattern, paths)?;

    assert!(result.is_some());
    assert!(result.unwrap().contains("Rust"));
    Ok(())
  }

  #[test]
  fn test_grep_basic_multi_line_match() -> Result<(), Box<dyn Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("multi_line.txt");
    write_to_file(&file_path, "Hello\nRust Language\nWorld\nRust Programming")?;

    let pattern = "Rust.*";
    let paths = vec![file_path];
    let result = grep(pattern, paths)?;

    assert!(result.is_some());
    let content = result.unwrap();
    assert!(content.contains("Rust Language"));
    assert!(content.contains("Rust Programming"));
    Ok(())
  }

  #[test]
  fn test_grep_pattern_not_found() -> Result<(), Box<dyn Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("not_found.txt");
    write_to_file(&file_path, "Hello\nRust\nWorld")?;

    let pattern = "Nonexistent";
    let paths = vec![file_path];
    let result = grep(pattern, paths)?;

    assert!(result.is_some());
    assert!(result.unwrap().contains("No matches found for pattern: Nonexistent"));
    Ok(())
  }

  #[test]
  fn test_grep_invalid_pattern() -> Result<(), Box<dyn Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("invalid_pattern.txt");
    write_to_file(&file_path, "Hello\nRust\nWorld")?;

    let pattern = "[Unclosed bracket";
    let paths = vec![file_path];
    let result = grep(pattern, paths)?;

    assert!(result.is_some());
    assert!(result.unwrap().contains("Error parsing grep pattern:"));
    Ok(())
  }

  #[test]
  fn test_grep_with_binary_file() -> Result<(), Box<dyn Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("binary_file.bin");
    let mut file = File::create(&file_path)?;
    file.write_all(&[0, 159, 146, 150])?; // Some arbitrary non-text bytes

    let pattern = ".*";
    let paths = vec![file_path];
    let result = grep(pattern, paths)?;

    assert!(result.is_some());
    assert!(result.unwrap().contains("No matches found for pattern: .*"));
    Ok(())
  }
}
