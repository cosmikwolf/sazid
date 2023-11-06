use std::{collections::HashMap, fs::File, io::Write};

use crate::app::session_config::SessionConfig;

use super::{
  types::{Command, CommandParameters, CommandProperty},
  FunctionCall, FunctionCallError,
};

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
  match File::create(path) {
    Ok(mut file) => match file.write_all(text.as_bytes()) {
      Ok(_) => Ok(Some("file created".to_string())),
      Err(e) => Ok(Some(format!("error writing file: {}", e))),
    },
    Err(e) => Ok(Some(format!("error creating file at {}, error: {}", path, e))),
  }
}
