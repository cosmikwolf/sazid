use std::{collections::HashMap, path::PathBuf};

use crate::{app::session_config::SessionConfig, trace_dbg};
use clap::Parser;
use serde_json::json;
use walkdir::WalkDir;

use super::{errors::ModelFunctionError, types::CommandProperty};

pub fn clap_args_to_json<P: Parser>() -> String {
  let app = P::command();
  let mut options = Vec::new();

  for arg in app.get_arguments() {
    if let Some(long) = arg.get_long() {
      let help = arg.get_help().map(|s| s.to_string()).unwrap_or_default();

      let entry = json!({ long.to_string(): help });
      options.push(entry);
    }
  }

  let t = serde_json::to_string_pretty(&options).unwrap();
  trace_dbg!("clap_args_to_json: {}", t);
  t
}

pub fn validate_and_extract_options<T>(
  function_name: String,
  function_args: &HashMap<String, serde_json::Value>,
  _required: bool,
) -> Result<Vec<String>, ModelFunctionError>
where
  T: Parser + std::fmt::Debug,
{
  trace_dbg!("function_args: {:?}", function_args);

  let mut function_args_string = function_args
    .iter()
    .flat_map(|(key, value)| {
      vec![format!("--{}", key), format!("{}", serde_json::from_value::<String>(value.clone()).unwrap_or_default())]
    })
    .collect::<Vec<String>>();
  function_args_string.insert(0, function_name);

  trace_dbg!("function_args_string: {:?}", function_args_string);

  match T::command().try_get_matches_from(function_args_string) {
    Ok(matches) => {
      trace_dbg!("matches 1: {:#?}", matches);

      let paths: Vec<Vec<&String>> = matches.get_occurrences("paths").unwrap().map(Iterator::collect).collect();
      let paths: Vec<&str> = paths.iter().flatten().map(|s| s.as_str()).collect();

      let mut matches =
        matches.ids().filter(|m| m.as_str() != "paths").map(|m| format!("--{}", m)).collect::<Vec<String>>();

      match validate_paths(None, SessionConfig::default(), paths) {
        Ok(paths) => {
          matches.extend(paths.iter().map(|p| p.to_str().unwrap().to_string()));
          trace_dbg!("matches: {:?}", matches);
          Ok(matches)
        },
        Err(err) => Err(ModelFunctionError::new(err.to_string().as_str())),
      }
    },
    Err(err) => return Err(ModelFunctionError::new(err.to_string().as_str())),
  }
}

pub fn generate_command_properties<T>(required: bool) -> Vec<CommandProperty>
where
  T: Parser + std::fmt::Debug,
{
  T::command()
    .get_arguments()
    .filter(|a| a.is_required_set() == required)
    .map(|a| {
      let property_type = a.get_value_names().unwrap_or(&[clap::builder::Str::from("string")])[0].to_string();

      CommandProperty {
        name: a.get_long().unwrap().to_string(),
        required: a.is_required_set(),
        property_type,
        description: Some(a.get_help().map(|s| s.to_string()).unwrap_or_default()),
        enum_values: None,
      }
    })
    .collect()
}

pub fn validate_and_extract_string_argument(
  function_args: &HashMap<String, serde_json::Value>,
  argument: &str,
  required: bool,
) -> Result<Option<String>, ModelFunctionError> {
  match function_args.get(argument) {
    Some(argument) => match argument {
      serde_json::Value::String(s) => Ok(Some(s.clone())),
      _ => Err(ModelFunctionError::new(format!("{} argument must be a string", argument).as_str())),
    },
    None => match required {
      true => Err(ModelFunctionError::new(format!("{} argument is required", argument).as_str())),
      false => Ok(None),
    },
  }
}

pub fn validate_paths(
  root_dir: Option<PathBuf>,
  session_config: SessionConfig,
  paths: Vec<&str>,
) -> Result<Vec<PathBuf>, ModelFunctionError> {
  let accesible_paths = get_accessible_file_paths(session_config.list_file_paths.clone(), root_dir);
  Ok(
    paths
      .iter()
      .map(|s| s.trim())
      .flat_map(|path| {
        let path_buf = PathBuf::from(path);
        if accesible_paths
          .contains_key(path_buf.to_str().ok_or_else(|| ModelFunctionError::new("Path contains invalid Unicode."))?)
        {
          Ok(path_buf)
        } else {
          Err(ModelFunctionError::new(&format!(
            "File path is not accessible: {:?}. Suggest using file_search command",
            path_buf
          )))
        }
      })
      .collect::<Vec<PathBuf>>(),
  )
}
pub fn validate_paths_csv(
  root_dir: Option<PathBuf>,
  session_config: SessionConfig,
  paths: &str,
) -> Result<Vec<PathBuf>, ModelFunctionError> {
  validate_paths(root_dir, session_config, paths.split(',').collect())
}

pub fn validate_and_extract_paths_from_argument(
  function_args: &HashMap<String, serde_json::Value>,
  session_config: SessionConfig,
  required: bool,
  root_dir: Option<PathBuf>,
) -> Result<Vec<PathBuf>, ModelFunctionError> {
  match function_args.get("paths") {
    Some(paths) => {
      if let serde_json::Value::String(paths_str) = paths {
        validate_paths_csv(root_dir, session_config, paths_str)
      } else {
        Err(ModelFunctionError::new("Expected a string for 'paths' argument but got a different type."))
      }
    },
    None if required => Err(ModelFunctionError::new("paths argument is required.")),
    None => Ok(vec![]),
  }
}

pub fn get_accessible_file_paths(list_file_paths: Vec<PathBuf>, root_dir: Option<PathBuf>) -> HashMap<String, PathBuf> {
  // Define the base directory you want to start the search from.
  let base_dir = match root_dir {
    Some(path) => path,
    None => PathBuf::from("./"),
  };

  // Create an empty HashMap to store the relative paths.
  let mut file_paths = HashMap::new();
  for mut path in list_file_paths {
    // Iterate through the files using WalkDir.
    path = base_dir.join(path);
    if path.exists() {
      WalkDir::new(path).into_iter().flatten().for_each(|entry| {
        let path = entry.path();
        file_paths.insert(path.to_string_lossy().to_string(), path.to_path_buf());
      });
    }
  }

  // WalkDir::new(base_dir).into_iter().flatten().for_each(|entry| {
  //   let path = entry.path();
  //   file_paths.insert(path.to_string_lossy().to_string(), path.to_path_buf());
  // });

  trace_dbg!("file_paths: {:?}", file_paths);
  file_paths
}

pub fn count_tokens(text: &str) -> usize {
  let bpe = tiktoken_rs::cl100k_base().unwrap();
  bpe.encode_with_special_tokens(text).len()
}
