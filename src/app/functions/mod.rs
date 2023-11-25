use crate::app::functions::function_call::ModelFunction;
use crate::{
  action::Action,
  app::messages::{ChatMessage, FunctionResult},
  trace_dbg,
};
use async_openai::types::ChatCompletionRequestFunctionMessage;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc::UnboundedSender;

use self::pcre2grep_function::Pcre2GrepFunction;
use self::{
  create_file_function::CreateFileFunction, errors::ModelFunctionError, file_search_function::FileSearchFunction,
  patch_files_function::PatchFileFunction, read_file_lines_function::ReadFileLinesFunction, types::Command,
};

use super::session_config::SessionConfig;

pub mod argument_validation;
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
  //Pcre2GrepFunction(Pcre2GrepFunction),
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
      //CallableFunction::Pcre2GrepFunction(f) => f.command_definition(),
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
    //CallableFunction::Pcre2GrepFunction(Pcre2GrepFunction::init()),
    CallableFunction::ReadFileLinesFunction(ReadFileLinesFunction::init()),
    // CallableFunction::ModifyFileFunction(ModifyFileFunction::init()),
    CallableFunction::CreateFileFunction(CreateFileFunction::init()),
    // CallableFunction::CargoCheckFunction(CargoCheckFunction::init()),
  ]
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
            //"pcre2grep" => Pcre2GrepFunction::init().call(function_args, session_config),
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
        tx.send(Action::AddMessage(ChatMessage::Function(ChatCompletionRequestFunctionMessage {
          name: fn_name,
          content: Some(output),
          ..Default::default()
        })))
        .unwrap();
      },
      Ok(None) => {},
      Err(e) => {
        // self.data.add_message(ChatMessage::FunctionResult(FunctionResult {
        //   name: fn_name,
        //   response: format!("Error: {:?}", e),
        // }));
        tx.send(Action::AddMessage(ChatMessage::Function(ChatCompletionRequestFunctionMessage {
          name: fn_name,
          content: Some(format!("Error: {:?}", e)),
          ..Default::default()
        })))
        .unwrap();
      },
    }
    tx.send(Action::RequestChatCompletion()).unwrap();
  });
}
