use crate::app::functions::function_call::FunctionCall;
use crate::components::Component;
use crate::{
  action::Action,
  app::messages::{ChatMessage, FunctionResult},
  trace_dbg,
};
use serde_derive::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use tokio::sync::mpsc::UnboundedSender;
use tracing_subscriber::util::SubscriberInitExt;
use walkdir::WalkDir;

use self::{
  cargo_check_function::CargoCheckFunction, create_file_function::CreateFileFunction, errors::FunctionCallError,
  file_search_function::FileSearchFunction, grep_function::GrepFunction, modify_file_function::ModifyFileFunction,
  patch_files_function::PatchFilesFunction, read_file_lines_function::ReadFileLinesFunction, types::Command,
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
pub mod read_file_lines_function;
pub mod types;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CallableFunction {
  FileSearchFunction(FileSearchFunction),
  GrepFunction(GrepFunction),
  ReadFileLinesFunction(ReadFileLinesFunction),
  ModifyFileFunction(ModifyFileFunction),
  CreateFileFunction(CreateFileFunction),
  PatchFilesFunction(PatchFilesFunction),
  CargoCheckFunction(CargoCheckFunction),
}

impl From<&CallableFunction> for Command {
  fn from(callable_function: &CallableFunction) -> Self {
    match callable_function {
      CallableFunction::FileSearchFunction(f) => f.command_definition(),
      CallableFunction::GrepFunction(f) => f.command_definition(),
      CallableFunction::ReadFileLinesFunction(f) => f.command_definition(),
      CallableFunction::ModifyFileFunction(f) => f.command_definition(),
      CallableFunction::CreateFileFunction(f) => f.command_definition(),
      CallableFunction::PatchFilesFunction(f) => f.command_definition(),
      CallableFunction::CargoCheckFunction(f) => f.command_definition(),
    }
  }
}

pub fn all_functions() -> Vec<CallableFunction> {
  vec![
    CallableFunction::PatchFilesFunction(PatchFilesFunction::init()),
    CallableFunction::FileSearchFunction(FileSearchFunction::init()),
    CallableFunction::GrepFunction(GrepFunction::init()),
    CallableFunction::ReadFileLinesFunction(ReadFileLinesFunction::init()),
    CallableFunction::ModifyFileFunction(ModifyFileFunction::init()),
    CallableFunction::CreateFileFunction(CreateFileFunction::init()),
    CallableFunction::CargoCheckFunction(CargoCheckFunction::init()),
  ]
}
// impl From<Box<dyn FunctionCall>> for RenderedFunctionCall {
//   fn from(function_call: Box<dyn FunctionCall>) -> Self {
//     RenderedFunctionCall { name: function_call.name, arguments: function_call.arguments }
//   }
// }

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
      let fn_args = fn_args;
      async move {
        let function_args_result: Result<HashMap<String, serde_json::Value>, serde_json::Error> =
          serde_json::from_str(fn_args.as_str());
        trace_dbg!("function call: {}\narguments:\n{:#?}", fn_name.clone(), function_args_result);
        match function_args_result {
          Ok(function_args) => match fn_name.as_str() {
            "create_file" => CreateFileFunction::init().call(function_args, session_config),
            "patch_files" => PatchFilesFunction::init().call(function_args, session_config),
            "grep" => GrepFunction::init().call(function_args, session_config),
            "file_search" => FileSearchFunction::init().call(function_args, session_config),
            "read_file" => ReadFileLinesFunction::init().call(function_args, session_config),
            "modify_file" => ModifyFileFunction::init().call(function_args, session_config),
            "cargo_check" => CargoCheckFunction::init().call(function_args, session_config),
            _ => Ok(Some("function not found".to_string())),
          },
          Err(e) => Err(FunctionCallError::new(
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
