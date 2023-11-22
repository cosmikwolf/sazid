use std::{collections::HashMap, path::PathBuf};

use crate::app::session_config::SessionConfig;
use serde_derive::{Deserialize, Serialize};

use super::{
  argument_validation::{validate_and_extract_boolean_argument, validate_and_extract_paths_from_argument},
  function_call::ModelFunction,
  types::{Command, CommandParameters, CommandProperty},
  ModelFunctionError,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatchFileFunction {
  name: String,
  description: String,
  required_properties: Vec<CommandProperty>,
  optional_properties: Vec<CommandProperty>,
}

impl ModelFunction for PatchFileFunction {
  fn init() -> Self {
    PatchFileFunction {
      name: "git_apply".to_string(),
      description: "modify files by executing 'git apply --index --3way <options> <paths>...'. git patch files must first be created with create_file, and must be in the git-format-patch format. all patch files should be created in ./.session_data/patches/".to_string(),
      required_properties: vec![
        CommandProperty {
          name: "reverse".to_string(),
          required: false,
          property_type: "boolean".to_string(),
          description: Some("Apply the patch in reverse".to_string()),
          enum_values: None,
        },
        CommandProperty {
          name: "paths".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some("git patch files to apply, comma separated".to_string()),
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
  ) -> Result<Option<String>, ModelFunctionError> {
    match validate_and_extract_paths_from_argument(&function_args, session_config, true, None) {
      Ok(Some(paths)) => match validate_and_extract_boolean_argument(&function_args, "reverse", false) {
        Ok(reverse) => execute_git_apply(reverse, paths),
        Err(err) => Ok(Some(err.to_string())),
      },
      Ok(None) => Ok(Some("no patch file passed to function".to_string())),
      Err(err) => Ok(Some(err.to_string())),
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

pub fn execute_git_apply(reverse: Option<bool>, paths: Vec<PathBuf>) -> Result<Option<String>, ModelFunctionError> {
  let output = std::process::Command::new("git")
    .arg("apply")
    .arg("--ignore-whitespace")
    .arg("--3way")
    .arg("--unidiff-zero")
    .arg("--inaccurate-eof")
    .arg("--unsafe-paths")
    .arg("--verbose")
    .args(if reverse.unwrap_or(false) { vec!["--reverse"] } else { vec![] })
    .args(paths)
    .output()
    .map_err(|e| ModelFunctionError::new(e.to_string().as_str()))?;

  if !output.status.success() {
    return Ok(Some(ModelFunctionError::new(&String::from_utf8_lossy(output.stderr.as_slice())).to_string()));
  }

  Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}
