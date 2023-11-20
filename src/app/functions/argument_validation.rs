use std::{collections::HashMap, path::PathBuf};

use crate::{app::session_config::SessionConfig, trace_dbg};
use clap::Parser;
use serde_json::json;
use walkdir::WalkDir;

use super::errors::ModelFunctionError;

pub fn clap_args_to_json<P: Parser>() -> String {
  let app = P::command();
  let mut options = Vec::new();

  for arg in app.get_arguments() {
    if let Some(short) = arg.get_short() {
      let help = arg.get_help().map(|s| s.to_string()).unwrap_or_default();

      let entry = json!({ short.to_string(): help });
      options.push(entry);
    }
  }

  serde_json::to_string_pretty(&options).unwrap()
}

pub fn validate_and_extract_options<T>(
  function_args: &HashMap<String, serde_json::Value>,
  required: bool,
) -> Result<Option<Vec<String>>, ModelFunctionError>
where
  T: Parser + std::fmt::Debug,
{
  match function_args.get("options") {
    Some(options) => match T::try_parse_from(options.as_str().unwrap().split(' ')) {
      Ok(args) => Ok(Some(format!("{:?}", args).split(',').map(|a| a.to_string()).collect())),
      Err(err) => Err(ModelFunctionError::new(err.to_string().as_str())),
    },
    None => match required {
      true => Err(ModelFunctionError::new("options argument is required")),
      false => Ok(None),
    },
  }
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
pub fn validate_and_extract_paths_from_argument(
  function_args: &HashMap<String, serde_json::Value>,
  session_config: SessionConfig,
  required: bool,
  root_dir: Option<PathBuf>,
) -> Result<Option<Vec<PathBuf>>, ModelFunctionError> {
  match function_args.get("paths") {
    Some(paths) => {
      if let serde_json::Value::String(paths_str) = paths {
        let accesible_paths = get_accessible_file_paths(session_config.list_file_paths.clone(), root_dir);
        let paths_vec: Result<Vec<PathBuf>, ModelFunctionError> = paths_str
          .split(',')
          .map(|s| s.trim())
          .map(|path| {
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
          .collect(); // Collect into a Result<Vec<PathBuf>, ModelFunctionError>
        paths_vec.map(Some)
      } else {
        Err(ModelFunctionError::new("Expected a string for 'paths' argument but got a different type."))
      }
    },
    None if required => Err(ModelFunctionError::new("paths argument is required.")),
    None => Ok(None),
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
