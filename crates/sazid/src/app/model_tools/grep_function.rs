use futures_util::Future;
use grep::{
  printer::StandardBuilder,
  regex::RegexMatcher,
  searcher::{BinaryDetection, SearcherBuilder},
};
use serde::{Deserialize, Serialize};

use std::{io::BufWriter, path::PathBuf};
use std::{io::Write, pin::Pin};
use walkdir::WalkDir;

use super::{
  argument_validation::{
    validate_and_extract_paths_from_argument,
    validate_and_extract_string_argument,
  },
  errors::ToolCallError,
  tool_call::{ToolCallParams, ToolCallTrait},
  types::{FunctionProperty, PropertyType},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrepFunction {
  pub name: String,
  pub description: String,
  pub properties: Vec<FunctionProperty>,
}

impl Default for GrepFunction {
  fn default() -> Self {
    GrepFunction {
      name: "grep".to_string(),
      description: "an implementation of grep".to_string(),
      properties: vec![
        FunctionProperty {
          name: "pattern".to_string(),
          required: true,
          property_type: PropertyType::String,
          description: Some("a regular expression pattern to match against file contents".to_string()),
          enum_values: None,
        },
        FunctionProperty {
          name: "paths".to_string(),
          required: true,
          property_type: PropertyType::String,
          description: Some(
            "a list of comma separated paths to walk for files which the pattern will be matched against".to_string(),
          ),
          enum_values: None,
        },
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
}

impl ToolCallTrait for GrepFunction {
  fn init() -> Self {
    GrepFunction::default()
  }
  fn name(&self) -> &str {
    &self.name
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
    let paths = validate_and_extract_paths_from_argument(
      &params.function_args,
      params.session_config,
      true,
      None,
    )
    .unwrap()
    .unwrap();
    let pattern = validate_and_extract_string_argument(
      &params.function_args,
      "pattern",
      true,
    )
    .unwrap()
    .unwrap();
    Box::pin(async move { grep(pattern.as_str(), paths) })
  }

  fn parameters(&self) -> Vec<FunctionProperty> {
    self.properties.clone()
  }

  fn description(&self) -> String {
    self.description.clone()
  }
}

pub fn grep(
  pattern: &str,
  paths: Vec<PathBuf>,
) -> Result<Option<String>, ToolCallError> {
  //let mut buffer = Cursor::new(Vec::new());
  let buffer: BufWriter<Vec<u8>> = BufWriter::new(Vec::new());
  let mut error_buffer: BufWriter<Vec<u8>> = BufWriter::new(Vec::new());
  match RegexMatcher::new(pattern) {
    Ok(matcher) => {
      let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .line_number(false)
        .build();
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
          let result = searcher.search_path(
            &matcher,
            dent.path(),
            printer.sink_with_path(&matcher, dent.path()),
          );
          if let Err(err) = result {
            error_buffer
              .write_all(
                format!("{}: {}", dent.path().display(), err).as_bytes(),
              )
              .unwrap();
          }
        }
      }
      match printer.into_inner().into_inner().into_inner() {
        Ok(matches) => {
          if matches.is_empty() {
            error_buffer
              .write_all(
                format!("No matches found for pattern: {}", pattern).as_bytes(),
              )
              .unwrap();
          }

          Ok(Some(
            String::from_utf8(matches)
              .unwrap_or_else(|_| "Error parsing grep output text".to_string())
              + String::from_utf8(error_buffer.into_inner().unwrap())
                .unwrap_or_else(|_| {
                  "Error parsing grep output text".to_string()
                })
                .as_str(),
          ))
        },
        Err(err) => {
          error_buffer
            .write_all(
              format!("Error parsing grep output text: {:?}", err).as_bytes(),
            )
            .unwrap();
          Ok(Some(
            String::from_utf8(error_buffer.into_inner().unwrap()).unwrap(),
          ))
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
  fn write_to_file(
    file_path: &PathBuf,
    content: &str,
  ) -> Result<(), std::io::Error> {
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
    assert!(result
      .unwrap()
      .contains("No matches found for pattern: Nonexistent"));
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
