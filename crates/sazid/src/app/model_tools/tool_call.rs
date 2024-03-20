use crate::{action::Action, app::messages::ChatMessage};
use async_openai::types::{
  ChatCompletionMessageToolCall, ChatCompletionRequestToolMessage,
  ChatCompletionTool, ChatCompletionToolType, FunctionObject, Role,
};
use serde_json::Value;
use std::{any::Any, collections::HashMap, pin::Pin};
use tokio::sync::mpsc::UnboundedSender;

use futures_util::Future;

use crate::app::session_config::SessionConfig;

use super::{
  cargo_check_function::CargoCheckFunction, errors::ToolCallError,
  file_search_function::FileSearchFunction,
  pcre2grep_function::Pcre2GrepFunction,
  read_file_lines_function::ReadFileLinesFunction, types::ToolCall,
};

pub trait ToolCallTrait: Any + Send + Sync {
  fn init() -> Self
  where
    Self: Sized;

  fn as_any(&self) -> &dyn Any
  where
    Self: Sized,
  {
    self
  }

  fn name(&self) -> &str;

  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Pin<
    Box<
      dyn Future<Output = Result<Option<String>, ToolCallError>>
        + Send
        + 'static,
    >,
  >;

  fn function_definition(&self) -> ToolCall;

  fn to_chat_completion_tool(
    &self,
  ) -> Result<ChatCompletionTool, ToolCallError> {
    let tool_call = self.function_definition();
    Ok(ChatCompletionTool {
      r#type: ChatCompletionToolType::Function,
      function: FunctionObject {
        name: tool_call.name,
        description: tool_call.description,
        parameters: tool_call
          .parameters
          .map(|p| serde_json::to_value(p).unwrap()),
      },
    })
  }
}

pub fn get_enabled_tools(
  enabled_tools: Option<Vec<String>>,
) -> Result<Option<Vec<ChatCompletionTool>>, ToolCallError> {
  let tools = enabled_tools_functions(enabled_tools)?;

  if tools.is_empty() {
    Ok(None)
  } else {
    Ok(Some(
      tools.iter().flat_map(|tool| tool.to_chat_completion_tool()).collect(),
    ))
  }
}

pub fn enabled_tools_functions(
  disabled_tools: Option<Vec<String>>,
) -> Result<Vec<Pin<Box<dyn ToolCallTrait + 'static>>>, ToolCallError> {
  let tool_functions: Vec<Pin<Box<dyn ToolCallTrait + 'static>>> = vec![
    Box::pin(CargoCheckFunction::init()),
    Box::pin(Pcre2GrepFunction::init()),
    Box::pin(FileSearchFunction::init()),
    Box::pin(ReadFileLinesFunction::init()),
  ];

  if let Some(disabled_tools) = &disabled_tools {
    for tool in disabled_tools {
      if !tool_functions
        .iter()
        .any(|tool_func| &tool_func.name().to_string() == tool)
      {
        return Err(ToolCallError::new(&format!(
          "disabled tool not found: {}",
          tool
        )));
      }
    }
    Ok(
      tool_functions
        .into_iter()
        .filter(|tool| !disabled_tools.contains(&tool.name().to_string()))
        .collect::<Vec<Pin<Box<dyn ToolCallTrait + 'static>>>>(),
    )
  } else {
    Ok(tool_functions)
  }
}

pub fn get_tool_by_name(
  tool_name: &str,
  disabled_tools: Option<Vec<String>>,
) -> Result<Pin<Box<dyn ToolCallTrait>>, ToolCallError> {
  log::debug!("disabled tools: {:?}", disabled_tools.clone());
  let tools = enabled_tools_functions(disabled_tools).unwrap();
  for tool in tools.iter() {
    log::debug!("tool: {}", tool.name());
  }
  match tools.into_iter().find(|tool| tool.name() == tool_name) {
    Some(tool) => Ok(tool),
    None => Err(ToolCallError::new(&format!(
      "Tool Call Error: tool not found: {}",
      tool_name
    ))),
  }
}

pub fn call_tool(
  tx: UnboundedSender<Action>,
  tool_name: String,
  tool_args: HashMap<String, Value>,
  tool_call_id: String,
  session_config: SessionConfig,
  session_id: i64,
) {
  match get_tool_by_name(
    tool_name.as_str(),
    Some(session_config.disabled_tools.clone()),
  ) {
    Ok(tool) => {
      tokio::spawn(async move {
        let tool_call_result = tool.call(tool_args, session_config).await;

        match tool_call_result {
          Ok(output) => {
            tx.send(Action::ToolCallComplete(session_id, tool_call_id, output))
              .unwrap();
          },
          Err(e) => {
            tx.send(Action::ToolCallError(
              session_id,
              tool_call_id,
              format!("Tool Call Error: {}\nTool Name: {}", e, tool_name),
            ))
            .unwrap();
          },
        }
      });
    },
    Err(e) => {
      tx.send(Action::ToolCallError(
        session_id,
        tool_call_id,
        format!("Tool Call Error: {}\nTool Name: {}", e, tool_name),
      ))
      .unwrap();
    },
  }
}

pub fn handle_tool_call_error(
  session_id: i64,
  tool_call_id: String,
  content: String,
  tx: UnboundedSender<Action>,
) {
  tx.send(Action::AddMessage(
    session_id,
    ChatMessage::Tool(ChatCompletionRequestToolMessage {
      tool_call_id,
      content,
      role: Role::Tool,
    }),
  ))
  .unwrap();
}

pub fn handle_tool_complete(
  session_id: i64,
  tool_call_id: String,
  output: Option<String>,
  tx: UnboundedSender<Action>,
) {
  tx.send(Action::AddMessage(
    session_id,
    ChatMessage::Tool(ChatCompletionRequestToolMessage {
      tool_call_id,
      content: output.unwrap_or("tool call complete".to_string()),
      role: Role::Tool,
    }),
  ))
  .unwrap();
  tx.send(Action::RequestChatCompletion()).unwrap();
}

pub fn handle_tool_call(
  tx: UnboundedSender<Action>,
  tool_call: &ChatCompletionMessageToolCall,
  session_config: SessionConfig,
  session_id: i64,
) {
  let function_args_result: Result<
    HashMap<String, serde_json::Value>,
    serde_json::Error,
  > = serde_json::from_str(tool_call.function.arguments.as_str());

  match function_args_result {
    Ok(function_args) => {
      call_tool(
        tx.clone(),
        tool_call.function.name.clone(),
        function_args,
        tool_call.id.clone(),
        session_config,
        session_id,
      );
    },
    Err(e) => {
      handle_tool_call_error(
        session_id,
        tool_call.id.clone(),
        format!(
          "Failed to parse function arguments:\nfunction:{:?}\nargs:{:?}\nerror:{:?}",
          tool_call.function.name, tool_call.function.arguments, e
        ),
        tx.clone(),
      );
    },
  }
}
