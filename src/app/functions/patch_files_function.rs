use std::{collections::HashMap, path::PathBuf};

use clap::Parser;

use crate::app::session_config::SessionConfig;
use serde_derive::{Deserialize, Serialize};

use super::{
  clap_args_to_json,
  function_call::ModelFunction,
  types::{Command, CommandParameters, CommandProperty},
  validate_and_extract_options, validate_and_extract_paths_from_argument, ModelFunctionError,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatchFileFunction {
  name: String,
  description: String,
  required_properties: Vec<CommandProperty>,
  optional_properties: Vec<CommandProperty>,
}

/// This command applies the changes from patch files to the repository.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
  #[clap(short = 'i', long = "ignore-whitespace", help = "Ignore white space when comparing.")]
  ignore_whitespace: bool,
  #[clap(short = 'w', long = "whitespace", help = "Warn about whitespace problems.")]
  whitespace: Option<String>,
  #[clap(short = 'R', long = "reject", help = "Leave rejected hunks in .rej files.")]
  reject: bool,
  #[clap(short = 'd', long = "directory", help = "Prepend <path> to all filenames.")]
  directory: Option<String>,
  #[clap(short = 'P', long = "unsafe-paths", help = "Allow applying of patches outside of the working area.")]
  unsafe_paths: bool,
  #[clap(short = 'r', long = "reverse", help = "Apply the patch in reverse.")]
  reverse: bool,
  #[clap(short = 'v', long = "verbose", help = "Provide verbose output.")]
  verbose: bool,
}
impl ModelFunction for PatchFileFunction {
  fn init() -> Self {
    PatchFileFunction {
      name: "git_apply".to_string(),
      description: "modify files using <git apply -i -3>".to_string(),
      required_properties: vec![
        CommandProperty {
          name: "options".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some(format!(
            "options to pass to git apply. --index and --3way are passed by default, options must be space separated. valid options: {}",
            clap_args_to_json::<Args>()
          )),
          enum_values: None,
        },
        CommandProperty {
          name: "patch_files".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some("git patch files to apply".to_string()),
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
    let options = validate_and_extract_options::<Args>(&function_args, false).unwrap().unwrap_or_default();
    let paths =
      validate_and_extract_paths_from_argument(&function_args, session_config, true).unwrap().unwrap_or_default();

    execute_git_apply(options, paths)
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

pub fn execute_git_apply(options: Vec<String>, paths: Vec<PathBuf>) -> Result<Option<String>, ModelFunctionError> {
  let output = std::process::Command::new("git")
    .arg("apply")
    .arg("--index")
    .arg("--3way")
    .args(options)
    .args(paths)
    .output()
    .map_err(|e| ModelFunctionError::new(e.to_string().as_str()))?;

  if !output.status.success() {
    return Ok(Some(ModelFunctionError::new(output.status.code().unwrap().to_string().as_str()).to_string()));
  }

  Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}
