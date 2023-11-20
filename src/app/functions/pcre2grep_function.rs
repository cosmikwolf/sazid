use crate::app::{functions::function_call::ModelFunction, session_config::SessionConfig};
use std::{collections::HashMap, path::PathBuf};

use super::{
  argument_validation::{
    clap_args_to_json, validate_and_extract_paths_from_argument, validate_and_extract_string_argument,
  },
  errors::ModelFunctionError,
  types::{Command, CommandParameters, CommandProperty},
};
use clap::Parser;
use serde_derive::{Deserialize, Serialize};

// #[derive(Parser, Debug)]
// #[clap(author, version, about, long_about = None)]
// struct Args {
//   #[clap(short = 'i', long = "ignore-case", help = "Ignore case distinctions in the pattern.")]
//   ignore_case: bool,
//   #[clap(short = 'v', long = "invert-match", help = "Select non-matching lines.")]
//   invert_match: bool,
//   #[clap(short = 'l', long = "files-with-matches", help = "Print only file names with matches.")]
//   files_with_matches: bool,
//   #[clap(short = 'c', long = "count", help = "Print only a count of matching lines per file.")]
//   count: bool,
//   #[clap(short = 'n', long = "line-number", help = "Print line number with output lines.")]
//   line_number: bool,
//   #[clap(short = 'e', long = "regexp", help = "Specify a pattern, may be used more than once.", number_of_values = 1)]
//   regexp: Vec<String>,
//   #[clap(short = 'r', long = "recursive", help = "Recursively scan sub-directories.")]
//   recursive: bool,
//   #[clap(short = 'H', long = "with-filename", help = "Force the prefixing of the file name on output.")]
//   with_filename: bool,
//   #[clap(short = 'h', long = "no-filename", help = "Suppress the prefixing of the file name on output.")]
//   no_filename: bool,
//   #[clap(short = 'o', long = "only-matching", help = "Show only the part of the line that matched.")]
//   only_matching: bool,
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pcre2GrepFunction {
  pub name: String,
  pub description: String,
  pub required_properties: Vec<CommandProperty>,
  pub optional_properties: Vec<CommandProperty>,
}

pub fn execute_pcre2grep(
  // options: Option<Vec<String>>,
  pattern: String,
  paths: Vec<PathBuf>,
) -> Result<Option<String>, ModelFunctionError> {
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
    .map_err(|e| ModelFunctionError::new(e.to_string().as_str()))?;

  if !output.status.success() {
    return Ok(Some(ModelFunctionError::new(output.status.code().unwrap().to_string().as_str()).to_string()));
  }

  Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}

impl ModelFunction for Pcre2GrepFunction {
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
  ) -> Result<Option<String>, ModelFunctionError> {
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
