use crate::{
  action::{ChatToolAction, SessionAction, ToolType},
  app::messages::ChatMessage,
};
use async_openai::types::{
  ChatCompletionMessageToolCall, ChatCompletionRequestToolMessage,
  ChatCompletionTool, ChatCompletionToolType, FunctionObject, Role,
};
use serde_json::Value;
use std::{any::Any, collections::HashMap, pin::Pin, sync::Arc};
use tokio::sync::mpsc::UnboundedSender;

use futures_util::Future;

use crate::app::session_config::SessionConfig;

use super::{
  cargo_check_function::CargoCheckFunction,
  errors::ToolCallError,
  file_search_function::FileSearchFunction,
  lsp_get_diagnostics::LspGetDiagnostics,
  lsp_get_workspace_files::LspGetWorkspaceFiles,
  lsp_goto_symbol_declaration::LspGotoSymbolDeclaration,
  lsp_goto_symbol_definition::LspGotoSymbolDefinition,
  lsp_goto_type_definition::LspGotoTypeDefinition,
  lsp_query_symbols::LspQuerySymbol,
  types::{FunctionParameters, FunctionProperty, ToolCall},
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
    params: ToolCallParams,
  ) -> Pin<
    Box<
      dyn Future<Output = Result<Option<String>, ToolCallError>>
        + Send
        + 'static,
    >,
  >;

  fn properties(&self) -> Vec<FunctionProperty>;

  fn description(&self) -> String;

  fn function_definition(&self) -> ToolCall {
    let mut properties: HashMap<String, FunctionProperty> = HashMap::new();

    self.properties().iter().filter(|p| p.required).for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.properties().iter().filter(|p| !p.required).for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    ToolCall {
      name: self.name().to_string(),
      description: Some(self.description()),
      parameters: Some(FunctionParameters {
        param_type: "object".to_string(),
        required: self
          .properties()
          .clone()
          .into_iter()
          .filter(|p| p.required)
          .map(|p| p.name)
          .collect(),
        properties,
      }),
    }
  }

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

pub struct ToolCallParams {
  pub function_args: HashMap<String, serde_json::Value>,
  pub tool_result: Option<String>,
  pub tool_call_id: String,
  pub session_id: i64,
  pub session_config: SessionConfig,
  pub tx: UnboundedSender<ChatToolAction>,
}

pub struct ChatTools {
  pub tx: UnboundedSender<ChatToolAction>,
  config: HashMap<i64, SessionConfig>,
  tools: Vec<Arc<dyn ToolCallTrait + 'static>>,
}

impl ChatTools {
  pub fn new(
    tx: UnboundedSender<ChatToolAction>,
    session_id: i64,
    session_config: SessionConfig,
  ) -> Self {
    let tools = Self::all_tools().unwrap();
    let mut config: HashMap<i64, SessionConfig> = HashMap::new();
    config.insert(session_id, session_config);

    Self { tx, config, tools }
  }

  pub fn all_tools(
  ) -> Result<Vec<Arc<dyn ToolCallTrait + 'static>>, ToolCallError> {
    Ok(vec![
      // Arc::new(CargoCheckFunction::init()),
      // Arc::new(Pcre2GrepFunction::init()),
      // Arc::new(LspQuerySymbol::init()),
      // Arc::new(FileSearchFunction::init()),
      // Arc::new(LspGetWorkspaceFiles::init()),
      Arc::new(LspGotoSymbolDefinition::init()),
      // Arc::new(LspGotoSymbolDeclaration::init()),
      // Arc::new(LspGotoTypeDefinition::init()),
      // Arc::new(LspGetDiagnostics::init()),
      // Arc::new(ReadFileLinesFunction::init()),
    ])
  }

  pub fn upsert_configs(&mut self, session_id: i64, config: SessionConfig) {
    self.config.insert(session_id, config);
  }

  pub fn handle_action(
    &mut self,
    action: ChatToolAction,
  ) -> Result<Option<ChatToolAction>, ToolCallError> {
    match action {
      ChatToolAction::UpdateConfig(session_id, session_config) => {
        self.upsert_configs(session_id, *session_config);
        Ok(None)
      },
      ChatToolAction::CallTool(tool_call, session_id) => {
        log::debug!(
          "calling tool - session_id: {} tool_call_id: {}\n{:#?}",
          session_id,
          tool_call.id.clone(),
          tool_call
        );
        self.handle_tool_call(&tool_call, session_id);
        Ok(None)
      },
      ChatToolAction::ToolListRequest(session_id) => {
        let tools = self
          .tools
          .iter()
          .map(|tool| tool.to_chat_completion_tool())
          .collect::<Result<Vec<ChatCompletionTool>, ToolCallError>>()?;
        // log::debug!("tools request: {:#?}", tools);

        Ok(Some(ChatToolAction::SessionAction(Box::new(
          SessionAction::UpdateToolList(session_id, tools),
        ))))
      },
      _ => Ok(None),
    }
  }

  fn send_chat_tool_error(
    tx: UnboundedSender<ChatToolAction>,
    error: &ToolCallError,
    session_and_tool_call_id: Option<(i64, String)>,
  ) {
    log::error!("Chat Tool Error: {}", error);
    tx.send(ChatToolAction::Error(format!("Chat Tool Error: {}", error)))
      .unwrap();
    if let Some((session_id, tool_call_id)) = session_and_tool_call_id {
      tx.send(ChatToolAction::SessionAction(Box::new(
        SessionAction::ToolCallError(
          ToolType::Generic(session_id, tool_call_id),
          format!("Tool Call Error: {}", error),
        ),
      )))
      .unwrap();
    }
  }

  pub fn get_enabled_chat_completion_tools(
    &self,
    session_id: i64,
  ) -> Result<Option<Vec<ChatCompletionTool>>, ToolCallError> {
    let tools: Vec<_> = match self.validate_session_tool_config(session_id) {
      Ok(config) => self
        .tools
        .iter()
        .filter(|tool| {
          !config.disabled_tools.contains(&tool.name().to_string())
        })
        .collect(),
      Err(e) => {
        Self::send_chat_tool_error(self.tx.clone(), &e, None);
        return Err(e);
      },
    };
    if tools.is_empty() {
      Ok(None)
    } else {
      let tools =
        tools.iter().flat_map(|tool| tool.to_chat_completion_tool()).collect();
      log::debug!("tools: {:#?}", tools);
      Ok(Some(tools))
    }
  }

  fn validate_session_tool_config(
    &self,
    session_id: i64,
  ) -> Result<&SessionConfig, ToolCallError> {
    let config = match self.config.get(&session_id) {
      Some(config) => config,
      None => {
        return Err(ToolCallError::new(
          format!(
            "session config not found.\nrequested id: {}\nconfig: {:#?}",
            session_id, self.config
          )
          .as_str(),
        ));
      },
    };

    for tool in config.disabled_tools.clone() {
      if !self
        .tools
        .iter()
        .any(|tool_func| *tool_func.name().to_string() == tool)
      {
        return Err(ToolCallError::new(&format!(
          "disabled tool not found: {}",
          tool
        )));
      }
    }
    Ok(config)
  }

  pub fn get_tool_by_name(
    &self,
    tool_name: &str,
    session_id: i64,
  ) -> Result<Option<Arc<dyn ToolCallTrait + 'static>>, ToolCallError> {
    match self.validate_session_tool_config(session_id) {
      Ok(config) => Ok(
        self
          .tools
          .iter()
          .filter(|tool| {
            !config.disabled_tools.contains(&tool.name().to_string())
          })
          .find(|tool| tool.name() == tool_name)
          .cloned(),
      ),
      Err(e) => Err(e),
    }
  }

  pub fn call_tool(
    &self,
    tool_name: String,
    tool_args: HashMap<String, Value>,
    tool_call_id: String,
    session_id: i64,
  ) {
    log::info!(
      "Calling Chat tool:\n{:?} {:?}\ntool call id: {:?}",
      tool_name,
      tool_args,
      tool_call_id
    );

    let session_config = match self.config.get(&session_id) {
      Some(config) => config.clone(),
      None => {
        Self::send_chat_tool_error(
          self.tx.clone(),
          &ToolCallError::new(
            "session config not found. session_id: {} tool_call_id: {}",
          ),
          Some((session_id, tool_call_id.clone())),
        );
        return;
      },
    };

    let tx = self.tx.clone();

    match self.get_tool_by_name(tool_name.as_str(), session_id) {
      Ok(Some(tool)) => {
        let tool_call_id = tool_call_id.clone();
        let tool = tool.clone();
        tokio::spawn(async move {
          let tool_call_result = tool
            .call(ToolCallParams {
              tx: tx.clone(),
              tool_result: None,
              function_args: tool_args,
              tool_call_id: tool_call_id.clone(),
              session_id,
              session_config,
            })
            .await;
          match tool_call_result {
            // if a tool call has some output, then the call is complete
            Ok(Some(output)) => {
              log::debug!("tool call complete: {:?}", output);
              tx.send(ChatToolAction::SessionAction(Box::new(
                SessionAction::ToolCallComplete(
                  ToolType::Generic(session_id, tool_call_id),
                  output,
                ),
              )))
              .unwrap();
            },
            // if the tool call is none, then another module is responsible for the completion
            Ok(None) => {},
            Err(e) => {
              Self::send_chat_tool_error(
                tx.clone(),
                &e,
                Some((session_id, tool_call_id)),
              );
            },
          }
        });
      },
      Ok(None) => {
        Self::send_chat_tool_error(
          tx.clone(),
          &ToolCallError::new(
            format!("Tool Call Error: Tool not found: {}", tool_name).as_str(),
          ),
          Some((session_id, tool_call_id)),
        );
      },
      Err(e) => {
        Self::send_chat_tool_error(
          tx.clone(),
          &e,
          Some((session_id, tool_call_id)),
        );
      },
    }
  }

  pub fn complete_tool_call(
    &self,
    tool_output: String,
    error_occured: bool,
    tool_call_id: String,
    session_id: i64,
  ) {
    match error_occured {
      false => {
        self
          .tx
          .send(ChatToolAction::SessionAction(Box::new(
            SessionAction::ToolCallComplete(
              ToolType::Generic(session_id, tool_call_id),
              tool_output,
            ),
          )))
          .unwrap();
      },
      true => {
        Self::send_chat_tool_error(
          self.tx.clone(),
          &ToolCallError::new(
            format!("Tool Call Error- output: {}", tool_output).as_str(),
          ),
          Some((session_id, tool_call_id)),
        );
      },
    }
  }

  pub fn handle_tool_complete(
    &self,
    session_id: i64,
    tool_call_id: String,
    output: Option<String>,
  ) {
    self
      .tx
      .send(ChatToolAction::SessionAction(Box::new(SessionAction::AddMessage(
        session_id,
        ChatMessage::Tool(ChatCompletionRequestToolMessage {
          tool_call_id,
          content: output.unwrap_or("tool call complete".to_string()),
          role: Role::Tool,
        }),
      ))))
      .unwrap();
    self
      .tx
      .send(ChatToolAction::SessionAction(Box::new(
        SessionAction::RequestChatCompletion(),
      )))
      .unwrap()
  }

  pub fn handle_tool_call(
    &self,
    tool_call: &ChatCompletionMessageToolCall,
    session_id: i64,
  ) {
    let function_args_result: Result<
      HashMap<String, serde_json::Value>,
      serde_json::Error,
    > = serde_json::from_str(tool_call.function.arguments.as_str());

    match function_args_result {
      Ok(function_args) => {
        log::debug!(
          "handle tool call: call id: {} session id: {}",
          tool_call.id.clone(),
          session_id
        );

        self.call_tool(
          tool_call.function.name.clone(),
          function_args,
          tool_call.id.clone(),
          session_id,
        );
      },
      Err(e) => {
        Self::send_chat_tool_error(
          self.tx.clone(),
          &ToolCallError::new( format!( "Failed to parse function arguments:\nfunction:{:?}\nargs:{:?}\nerror:{:?}", tool_call.function.name, tool_call.function.arguments, e).as_str()),
          Some((session_id, tool_call.id.clone())),
        );
      },
    }
  }
}
