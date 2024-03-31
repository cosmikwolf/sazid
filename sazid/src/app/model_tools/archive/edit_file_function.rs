use std::fs::OpenOptions;
use std::io::BufRead;
use std::path::PathBuf;
use std::{collections::HashMap, fs::File, io::Write};

use crate::app::session_config::SessionConfig;
use crate::trace_dbg;

use super::argument_validation::*;
use super::tool_call::ToolCallTrait;
use super::{
  errors::ToolCallError,
  types::{FunctionParameters, FunctionProperties, ToolCall},
};

use futures_util::Future;
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct EditFileFunction {
  name: String,
  description: String,
  required_properties: Vec<FunctionProperties>,
  optional_properties: Vec<FunctionProperties>,
}

impl ToolCallTrait for EditFileFunction {
  fn init() -> Self {
    EditFileFunction {
      name: "edit_file".to_string(),
      description: "insert text into a file at a line and column".to_string(),
      required_properties: vec![
        FunctionProperties {
          name: "path".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some("path to file".to_string()),
          enum_values: None,
        },
        FunctionProperties {
          name: "line_num".to_string(),
          required: true,
          property_type: "number".to_string(),
          description: Some(
            "line number in file to start edit, 0 indexed".to_string(),
          ),
          enum_values: None,
        },
        FunctionProperties {
          name: "col".to_string(),
          required: true,
          property_type: "number".to_string(),
          description: Some(
            "column index in line to begin edit, 0 indexed".to_string(),
          ),
          enum_values: None,
        },
        FunctionProperties {
          name: "del_count".to_string(),
          required: false,
          property_type: "number".to_string(),
          description: Some(
            "number of characters to delete prior to inserting text"
              .to_string(),
          ),
          enum_values: None,
        },
        FunctionProperties {
          name: "text".to_string(),
          required: false,
          property_type: "string".to_string(),
          description: Some("text to insert".to_string()),
          enum_values: None,
        },
      ],
      optional_properties: vec![],
    }
  }

  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Pin< Box< dyn Future<Output = Result<Option<String>, ToolCallError>> + Send + 'static, >, > {
    async move {
      let path = validate_and_extract_path_from_argument(
        &function_args,
        session_config,
        true,
        Some(PathBuf::from("./")),
      )?
      .unwrap();
      let line = validate_and_extract_numeric_argument(
        &function_args,
        "line_num",
        true,
      )?
      .unwrap() as usize;
      let col =
        validate_and_extract_numeric_argument(&function_args, "col", true)?
          .unwrap() as usize;
      let del_count = validate_and_extract_numeric_argument(
        &function_args,
        "del_count",
        true,
      )?
      .unwrap() as usize;
      let text =
        validate_and_extract_string_argument(&function_args, "text", true)?
          .unwrap();
      edit_file(&path, line, col, del_count, &text)
    }
  }

  fn function_definition(&self) -> ToolCall {
    let mut properties: HashMap<String, FunctionProperties> = HashMap::new();

    self.required_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.optional_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    ToolCall {
      name: self.name.clone(),
      description: Some(self.description.clone()),
      parameters: Some(FunctionParameters {
        param_type: "object".to_string(),
        required: self
          .required_properties
          .clone()
          .into_iter()
          .map(|p| p.name)
          .collect(),
        properties,
      }),
    }
  }
}

pub fn edit_file(
  path: &PathBuf,
  line_num: usize,
  col: usize,
  del_count: usize,
  text: &str,
) -> Result<Option<String>, ToolCallError> {
  match File::open(path) {
    Ok(file) => {
      let reader = std::io::BufReader::new(file);
      let original_lines: Vec<String> = reader
        .lines()
        .map(|line| line.unwrap_or_default().to_string())
        .collect();
      let mut new_lines: Vec<String> = original_lines.clone();
      // insert text at line and col
      for (line_index, line) in original_lines.iter().enumerate() {
        if line_index == line_num {
          let mut new_line = line.clone();
          new_line.replace_range(col..col + del_count, text);
          new_lines[line_num] = new_line;
          break;
        }
      }

      match OpenOptions::new().write(true).truncate(true).open(path) {
        Ok(mut file) => match file.write_all(new_lines.join("\n").as_bytes()) {
          Ok(_) => {
            let diff_result =
              diff::lines(&original_lines.join("\n"), &new_lines.join("\n"))
                .iter()
                .map(|d| match d {
                  diff::Result::Left(l) => format!("- {}", l),
                  diff::Result::Right(r) => format!("+ {}", r),
                  diff::Result::Both(b, _) => format!("  {}", b),
                })
                .collect::<Vec<String>>()
                .join("\n");
            let changed_char_count = new_lines.join("\n").chars().count()
              - original_lines.join("\n").chars().count();
            Ok(Some(format!(
              "{} chars added. diff of changes: {}",
              changed_char_count, diff_result
            )))
          },
          Err(e) => {
            let message = format!("error inserting text: {}", e);
            trace_dbg!("{}", message);
            Ok(Some(message))
          },
        },
        Err(e) => Ok(Some(format!(
          "error opening file for writing at {:#?}, error: {}",
          path, e
        ))),
      }
    },
    Err(e) => Ok(Some(format!(
      "error opening file for reading at {:#?}, error: {}",
      path, e
    ))),
  }
}
