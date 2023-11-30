use crate::app::functions::tool_call::ToolCallTrait;
use crate::{action::Action, app::messages::ChatMessage, trace_dbg};
use async_openai::types::{ChatCompletionMessageToolCall, ChatCompletionRequestToolMessage, Role};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc::UnboundedSender;

use self::modify_file_function::ModifyFileFunction;
use self::{
  create_file_function::CreateFileFunction, errors::ToolCallError, file_search_function::FileSearchFunction,
  read_file_lines_function::ReadFileLinesFunction, types::FunctionCall,
};

use super::session_config::SessionConfig;

pub mod argument_validation;
pub mod cargo_check_function;
pub mod create_file_function;
pub mod errors;
pub mod file_search_function;
pub mod grep_function;
pub mod modify_file_function;
pub mod patch_files_function;
pub mod pcre2grep_function;
pub mod read_file_lines_function;
pub mod tool_call;
pub mod tool_call_template;
pub mod types;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CallableFunction {
  FileSearchFunction(FileSearchFunction),
  //Pcre2GrepFunction(Pcre2GrepFunction),
  //GrepFunction(GrepFunction),
  ReadFileLinesFunction(ReadFileLinesFunction),
  ModifyFileFunction(ModifyFileFunction),
  CreateFileFunction(CreateFileFunction),
  //PatchFileFunction(PatchFileFunction),
  //CargoCheckFunction(CargoCheckFunction),
}

impl From<&CallableFunction> for FunctionCall {
  fn from(callable_function: &CallableFunction) -> Self {
    match callable_function {
      CallableFunction::FileSearchFunction(f) => f.function_definition(),
      //CallableFunction::Pcre2GrepFunction(f) => f.command_definition(),
      CallableFunction::ReadFileLinesFunction(f) => f.function_definition(),
      CallableFunction::ModifyFileFunction(f) => f.function_definition(),
      CallableFunction::CreateFileFunction(f) => f.function_definition(),
      //CallableFunction::PatchFileFunction(f) => f.command_definition(),
      // CallableFunction::CargoCheckFunction(f) => f.command_definition(),
    }
  }
}

pub fn all_functions() -> Vec<CallableFunction> {
  vec![
    //CallableFunction::PatchFileFunction(PatchFileFunction::init()),
    CallableFunction::FileSearchFunction(FileSearchFunction::init()),
    //CallableFunction::Pcre2GrepFunction(Pcre2GrepFunction::init()),
    CallableFunction::ReadFileLinesFunction(ReadFileLinesFunction::init()),
    // CallableFunction::ModifyFileFunction(ModifyFileFunction::init()),
    CallableFunction::CreateFileFunction(CreateFileFunction::init()),
    // CallableFunction::CargoCheckFunction(CargoCheckFunction::init()),
  ]
}

pub fn handle_tool_call(
  tx: UnboundedSender<Action>,
  tool_call: &ChatCompletionMessageToolCall,
  session_config: SessionConfig,
) {
  let fn_name = tool_call.function.name.clone();
  let fn_args = tool_call.function.arguments.clone();
  let tc_clone = tool_call.clone();
  tokio::spawn(async move {
    match {
      async move {
        let function_args_result: Result<HashMap<String, serde_json::Value>, serde_json::Error> =
          serde_json::from_str(fn_args.as_str());
        trace_dbg!("tool call: {}\narguments:\n{:#?}", fn_name.clone(), function_args_result);
        match function_args_result {
          Ok(function_args) => match fn_name.as_str() {
            "create_file" => CreateFileFunction::init().call(function_args, session_config),
            //"git_apply" => PatchFileFunction::init().call(function_args, session_config),
            //"grep" => GrepFunction::init().call(function_args, session_config),
            "file_search" => FileSearchFunction::init().call(function_args, session_config),
            "read_file" => ReadFileLinesFunction::init().call(function_args, session_config),
            //"modify_file" => ModifyFileFunction::init().call(function_args, session_config),
            //"cargo_check" => CargoCheckFunction::init().call(function_args, session_config),
            //"pcre2grep" => Pcre2GrepFunction::init().call(function_args, session_config),
            _ => Ok(Some("function not found".to_string())),
          },
          Err(e) => Err(ToolCallError::new(
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
        trace_dbg!("tool output:\n{}", output);
        tx.send(Action::AddMessage(ChatMessage::Tool(ChatCompletionRequestToolMessage {
          tool_call_id: tc_clone.id,
          content: Some(output),
          role: Role::Tool,
        })))
        .unwrap();
      },
      Ok(None) => {},
      Err(e) => {
        // self.data.add_message(ChatMessage::FunctionResult(FunctionResult {
        //   name: fn_name,
        //   response: format!("Error: {:?}", e),
        // }));
        tx.send(Action::AddMessage(ChatMessage::Tool(ChatCompletionRequestToolMessage {
          tool_call_id: tc_clone.id,
          content: Some(format!("Error: {:?}", e)),
          role: Role::Tool,
        })))
        .unwrap();
      },
    }
    tx.send(Action::RequestChatCompletion()).unwrap();
  });
}
