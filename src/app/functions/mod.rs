use crate::app::functions::function_call::ModelFunction;
use crate::{
  action::Action,
  app::messages::{ChatMessage, FunctionResult},
  trace_dbg,
};
use clap::Parser;
use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use std::{collections::HashMap, path::PathBuf};
use tokio::sync::mpsc::UnboundedSender;
use walkdir::WalkDir;

use self::pcre2grep_function::Pcre2GrepFunction;
use self::{
  create_file_function::CreateFileFunction, errors::ModelFunctionError, file_search_function::FileSearchFunction,
  patch_files_function::PatchFileFunction, read_file_lines_function::ReadFileLinesFunction, types::Command,
};

use super::session_config::SessionConfig;

pub mod cargo_check_function;
pub mod create_file_function;
pub mod errors;
pub mod file_search_function;
pub mod function_call;
pub mod grep_function;
pub mod modify_file_function;
pub mod patch_files_function;
pub mod pcre2grep_function;
pub mod read_file_lines_function;
pub mod types;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CallableFunction {
  FileSearchFunction(FileSearchFunction),
  Pcre2GrepFunction(Pcre2GrepFunction),
  //GrepFunction(GrepFunction),
  ReadFileLinesFunction(ReadFileLinesFunction),
  //ModifyFileFunction(ModifyFileFunction),
  CreateFileFunction(CreateFileFunction),
  PatchFileFunction(PatchFileFunction),
  //CargoCheckFunction(CargoCheckFunction),
}

impl From<&CallableFunction> for Command {
  fn from(callable_function: &CallableFunction) -> Self {
    match callable_function {
      CallableFunction::FileSearchFunction(f) => f.command_definition(),
      CallableFunction::Pcre2GrepFunction(f) => f.command_definition(),
      CallableFunction::ReadFileLinesFunction(f) => f.command_definition(),
      // CallableFunction::ModifyFileFunction(f) => f.command_definition(),
      CallableFunction::CreateFileFunction(f) => f.command_definition(),
      CallableFunction::PatchFileFunction(f) => f.command_definition(),
      // CallableFunction::CargoCheckFunction(f) => f.command_definition(),
    }
  }
}

pub fn all_functions() -> Vec<CallableFunction> {
  vec![
    CallableFunction::PatchFileFunction(PatchFileFunction::init()),
    CallableFunction::FileSearchFunction(FileSearchFunction::init()),
    CallableFunction::Pcre2GrepFunction(Pcre2GrepFunction::init()),
    CallableFunction::ReadFileLinesFunction(ReadFileLinesFunction::init()),
    // CallableFunction::ModifyFileFunction(ModifyFileFunction::init()),
    CallableFunction::CreateFileFunction(CreateFileFunction::init()),
    // CallableFunction::CargoCheckFunction(CargoCheckFunction::init()),
  ]
}

fn clap_args_to_json<P: Parser>() -> String {
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

fn validate_and_extract_options<T>(
  function_args: &HashMap<String, serde_json::Value>,
  required: bool,
) -> Result<Option<Vec<String>>, ModelFunctionError>
where
  T: Parser + std::fmt::Debug,
{
  match function_args.get("options") {
    Some(options) => match T::try_parse_from(options.as_str().unwrap().split(' ')) {
      Ok(args) => Ok(Some(format!("{:?}", args).split(' ').map(|a| a.to_string()).collect())),
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
) -> Result<Option<Vec<PathBuf>>, ModelFunctionError> {
  match function_args.get("paths") {
    Some(paths) => {
      let mut paths_vec: Vec<PathBuf> = Vec::new();
      if let serde_json::Value::String(paths) = paths {
        for path in paths.split(',').map(|s| s.trim()) {
          let accesible_paths = get_accessible_file_paths(session_config.list_file_paths.clone());
          if !accesible_paths.contains_key(Path::new(path).to_str().unwrap()) {
            return Err(ModelFunctionError::new(
              format!("File path is not accessible: {:?}. Suggest using file_search command", path).as_str(),
            ));
          } else {
            paths_vec.push(path.into());
          }
        }
      }
      Ok(Some(paths_vec))
    },
    None => match required {
      true => Err(ModelFunctionError::new("paths argument is required")),
      false => Ok(None),
    },
  }
}

pub fn get_accessible_file_paths(list_file_paths: Vec<PathBuf>) -> HashMap<String, PathBuf> {
  // Define the base directory you want to start the search from.
  let base_dir = PathBuf::from("./");

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

pub fn handle_chat_response_function_call(
  tx: UnboundedSender<Action>,
  fn_name: String,
  fn_args: String,
  session_config: SessionConfig,
) {
  tokio::spawn(async move {
    match {
      let fn_name = fn_name.clone();
      let fn_args = fn_args.clone();
      async move {
        let function_args_result: Result<HashMap<String, serde_json::Value>, serde_json::Error> =
          serde_json::from_str(fn_args.as_str());
        trace_dbg!("function call: {}\narguments:\n{:#?}", fn_name.clone(), function_args_result);
        match function_args_result {
          Ok(function_args) => match fn_name.as_str() {
            "create_file" => CreateFileFunction::init().call(function_args, session_config),
            "git_apply" => PatchFileFunction::init().call(function_args, session_config),
            //"grep" => GrepFunction::init().call(function_args, session_config),
            "file_search" => FileSearchFunction::init().call(function_args, session_config),
            "read_file" => ReadFileLinesFunction::init().call(function_args, session_config),
            //"modify_file" => ModifyFileFunction::init().call(function_args, session_config),
            //"cargo_check" => CargoCheckFunction::init().call(function_args, session_config),
            "pcre2grep" => Pcre2GrepFunction::init().call(function_args, session_config),
            _ => Ok(Some("function not found".to_string())),
          },
          Err(e) => Err(ModelFunctionError::new(
            format!("Failed to parse function arguments:\nfunction:{:?}\nargs:{:?}\nerror:{:?}", fn_name, fn_args, e)
              .as_str(),
          )),
        }
      }
    }
    .await
    {
      Ok(Some(output)) => {
        //self.data.add_message(ChatMessage::FunctionResult(FunctionResult { name: fn_name, response: output }));
        trace_dbg!("function output:\n{}", output);
        tx.send(Action::AddMessage(ChatMessage::FunctionResult(FunctionResult { name: fn_name, response: output })))
          .unwrap();
      },
      Ok(None) => {},
      Err(e) => {
        // self.data.add_message(ChatMessage::FunctionResult(FunctionResult {
        //   name: fn_name,
        //   response: format!("Error: {:?}", e),
        // }));
        tx.send(Action::AddMessage(ChatMessage::FunctionResult(FunctionResult {
          name: fn_name,
          response: format!("Error: {:?}", e),
        })))
        .unwrap();
      },
    }
    tx.send(Action::RequestChatCompletion()).unwrap();
  });
}
