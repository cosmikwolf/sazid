use crate::app::{functions::tool_call::ToolCallTrait, session_config::SessionConfig};
use std::{collections::HashMap, path::PathBuf};

use super::{
  argument_validation::{validate_and_extract_paths_from_argument, validate_and_extract_string_argument},
  errors::ToolCallError,
  types::{FunctionCall, FunctionParameters, FunctionProperties},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pcre2GrepFunction {
  pub name: String,
  pub description: String,
  pub required_properties: Vec<FunctionProperties>,
  pub optional_properties: Vec<FunctionProperties>,
}

pub fn execute_pcre2grep(
  // options: Option<Vec<String>>,
  pattern: String,
  paths: Vec<PathBuf>,
) -> Result<Option<String>, ToolCallError> {
  let output = std::process::Command::new("pcre2grep")
    // .args({
    //   if let Some(options) = options {
    //     options
    //   } else {
    //     vec![]
    //   }
    // })
    .arg(pattern)
    .args(paths)
    .output()
    .map_err(|e| ToolCallError::new(e.to_string().as_str()))?;

  if !output.status.success() {
    return Ok(Some(ToolCallError::new(output.status.code().unwrap().to_string().as_str()).to_string()));
  }

  Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}

impl ToolCallTrait for Pcre2GrepFunction {
  fn init() -> Self {
    Pcre2GrepFunction {
      name: "pcre2grep".to_string(),
      description: "an implementation of grep".to_string(),
      required_properties: vec![
        // CommandProperty {
        //   name: "options".to_string(),
        //   required: true,
        //   property_type: "string".to_string(),
        //   description: Some(format!(
        //     "pcre2grep arguments, space separated. valid options: {}",
        //     clap_args_to_json::<Args>()
        //   )),
        //   enum_values: None,
        // },
        FunctionProperties {
          name: "pattern".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some("a regular expression pattern to match against file contents".to_string()),
          enum_values: None,
        },
        FunctionProperties {
          name: "paths".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some(
            "a list of comma separated paths to walk for files which the pattern will be matched against".to_string(),
          ),
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
  ) -> Result<Option<String>, ToolCallError> {
    match validate_and_extract_paths_from_argument(&function_args, session_config, true, None) {
      Ok(Some(paths)) => match validate_and_extract_string_argument(&function_args, "pattern", true) {
        Ok(Some(pattern)) => execute_pcre2grep(pattern, paths),
        Ok(None) => Ok(Some("pattenr is required".to_string())),
        Err(err) => Ok(Some(err.to_string())),
      },
      Ok(None) => Ok(Some("paths are required".to_string())),
      Err(err) => Ok(Some(err.to_string())),
    }
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
