use std::fs::OpenOptions;
use std::io::BufRead;
use std::{collections::HashMap, fs::File, io::Write};

use crate::app::session_config::SessionConfig;
use crate::trace_dbg;

use super::{
  types::{Command, CommandParameters, CommandProperty},
  FunctionCall, FunctionCallError,
};

pub struct ModifyFileFunction {
  name: String,
  description: String,
  required_properties: Vec<CommandProperty>,
  optional_properties: Vec<CommandProperty>,
}

impl FunctionCall for ModifyFileFunction {
  fn init() -> Self {
    ModifyFileFunction {
      name: "modify_file".to_string(),
      description: "modify a file at path with text".to_string(),
      required_properties: vec![
        CommandProperty {
          name: "path".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some("path to file".to_string()),
          enum_values: None,
        },
        CommandProperty {
          name: "start_line".to_string(),
          required: true,
          property_type: "number".to_string(),
          description: Some("line to start modification".to_string()),
          enum_values: None,
        },
      ],
      optional_properties: vec![
        CommandProperty {
          name: "end_line".to_string(),
          required: false,
          property_type: "number".to_string(),
          description: Some("line to end modification".to_string()),
          enum_values: None,
        },
        CommandProperty {
          name: "insert_text".to_string(),
          required: false,
          property_type: "string".to_string(),
          description: Some("text to insert at start_line".to_string()),
          enum_values: None,
        },
      ],
    }
  }

  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    _session_config: SessionConfig,
  ) -> Result<Option<String>, FunctionCallError> {
    let path: Option<&str> = function_args.get("path").and_then(|s| s.as_str());
    let start_line = match function_args.get("start_line").and_then(|s| s.as_u64().map(|u| u as usize)) {
      Some(start_line) => start_line,
      None => return Err(FunctionCallError::new("start_line argument is required")),
    };

    let end_line: Option<usize> = function_args.get("end_line").and_then(|s| s.as_u64().map(|u| u as usize));
    let insert_text: Option<&str> = function_args.get("insert_text").and_then(|s| s.as_str());
    if let Some(path) = path {
      modify_file(path, start_line, end_line, insert_text)
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

pub fn modify_file(
  path: &str,
  start_line: usize,
  end_line: Option<usize>,
  insert_text: Option<&str>,
) -> Result<Option<String>, FunctionCallError> {
  match File::open(path) {
    Ok(file) => {
      let reader = std::io::BufReader::new(file);
      let mut new_lines: Vec<String> = reader.lines().map(|line| line.unwrap_or_default().to_string()).collect();
      let file_line_count = new_lines.len();
      if let Some(end_line) = end_line {
        if end_line > file_line_count {
          return Ok(Some("start_line + remove_line_count exceeds file length".to_string()));
        } else {
          // remove lines
          new_lines = new_lines
            .iter()
            .enumerate()
            .filter(|&(i, _)| i + 1 < start_line || i + 1 > end_line)
            .map(|(_, line)| line.to_string())
            .collect::<Vec<String>>();
        }
      }
      if let Some(insert_text) = insert_text {
        new_lines.insert(start_line, insert_text.to_string());
      }
      match OpenOptions::new().write(true).truncate(true).open(path) {
        Ok(mut file) => match file.write_all(new_lines.join("\n").as_bytes()) {
          Ok(_) => {
            let removed_string = if let Some(end_line) = end_line {
              format!("{} lines removed ", end_line - start_line)
            } else {
              "".to_string()
            };
            let added_string = if let Some(insert_text) = insert_text {
              format!("{} lines added ", insert_text.lines().count())
            } else {
              "".to_string()
            };
            Ok(Some(format!("success! {}{}at {}", removed_string, added_string, path)))
          },
          Err(e) => {
            let message = format!("error writing file modifications: {}", e);
            trace_dbg!("{}", message);
            Ok(Some(message))
          },
        },
        Err(e) => Ok(Some(format!("error opening file at {}, error: {}", path, e))),
      }
    },
    Err(e) => Ok(Some(format!("error opening file at {}, error: {}", path, e))),
  }
}
