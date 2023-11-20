use std::{collections::HashMap, path::PathBuf};

use clap::{ArgMatches, Parser};

use crate::{
  app::{functions::argument_validation::validate_paths, session_config::SessionConfig},
  trace_dbg,
};
use serde_derive::{Deserialize, Serialize};

use super::{
  argument_validation::{
    generate_command_properties, validate_and_extract_options, validate_and_extract_paths_from_argument,
  },
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

/// This command applies the changes from patch files to the repository.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
  #[clap(
    short = 'i',
    required = false,
    long = "ignore-whitespace",
    value_name = "boolean",
    help = "Ignore white space when comparing."
  )]
  ignore_whitespace: bool,
  #[clap(
    short = 'r',
    required = false,
    long = "reverse",
    value_name = "boolean",
    help = "Apply the patch in reverse."
  )]
  reverse: bool,
  #[clap(short = 'v', required = false, long = "verbose", value_name = "boolean", help = "Provide verbose output.")]
  verbose: bool,
  #[clap(
    short = 'p',
    required = true,
    long = "paths",
    value_name = "string",
    help = "git patch files to apply, comma separated"
  )]
  paths: String,
}

impl ModelFunction for PatchFileFunction {
  fn init() -> Self {
    PatchFileFunction {
      name: "git_apply".to_string(),
      description: "modify files by executing 'git apply --index --3way <options> <paths>...'. git patch files must first be created with create_file, and must be in the git-format-patch format. all patch files should be created in ./.session_data/patches/".to_string(),
      required_properties: generate_command_properties::<Args>(true),
      optional_properties:  generate_command_properties::<Args>(false),
    }
  }

  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Result<Option<String>, ModelFunctionError> {
    trace_dbg!("function_args: {:?}", function_args);

    match validate_and_extract_paths_from_argument(&function_args, session_config, true, None) {
      Ok(paths) => {
        trace_dbg!("paths: {:?}", paths);
        if paths.is_empty() {
          Ok(Some("no paths provided".to_string()))
        } else {
          match validate_and_extract_options::<Args>(self.name.clone(), &function_args, false) {
            Ok(options) => match validate_paths(None, SessionConfig::default(), paths) {
              Ok(paths) => execute_git_apply(options, paths),
              Err(err) => Err(ModelFunctionError::new(err.to_string().as_str())),
            },
            Err(err) => Ok(Some(err.to_string())),
          }
        }
      },
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

pub fn execute_git_apply(options: Vec<String>, paths: Vec<PathBuf>) -> Result<Option<String>, ModelFunctionError> {
  let output = std::process::Command::new("git")
    .arg("apply")
    .arg("--index")
    .arg("--3way")
    .args(options)
    .args(paths)
    .output()
    .map_err(|e| ModelFunctionError::new(e.to_string().as_str()))?;
  trace_dbg!("output: {:#?}", output);

  if !output.status.success() {
    return Ok(Some(ModelFunctionError::new(&String::from_utf8_lossy(&output.stderr)).to_string()));
  }

  Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}
