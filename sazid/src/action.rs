use std::path::PathBuf;

use crate::{
  app::{
    database::types::QueryableSession,
    lsi::query::LsiQuery,
    messages::ChatMessage,
    session_config::{SessionConfig, WorkspaceParams},
  },
  components::data_manager::DataManagerAction,
};
use async_openai::types::{
  ChatCompletionMessageToolCall, ChatCompletionRequestMessage, ChatCompletionTool,
};
use helix_lsp::Call;
use lsp_types::{DocumentSymbol, TextDocumentIdentifier};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolType {
  LsiQuery(LsiQuery),
  Generic(i64, String),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SessionAction {
  LspCheckServerNotifications,
  LspServerMessageReceived((usize, Call)),
  LspSymbolQuery(LsiQuery),
  CreateSession(SessionConfig),
  LoadSession(i64),
  SetTestToolResponse(ToolType, String),
  ToolCallComplete(ToolType, String),
  ToolCallError(ToolType, String),

  CreateLoadSessionResponse(QueryableSession),
  AddMessageEmbedding(i64, i64, ChatCompletionRequestMessage),
  MessageEmbeddingSuccess(i64),
  RequestRelatedMessages(i64, String, bool),
  SubmitInput(String),
  ExecuteCommand(String),
  CommandResult(String),
  RequestChatCompletion(),
  AddMessage(i64, ChatMessage),
  UpdateMessage(ChatCompletionRequestMessage, i64),
  ReloadMessages(Vec<(i64, ChatCompletionRequestMessage)>),
  UpdateStatus(Option<String>),
  UpdateToolList(i64, Vec<ChatCompletionTool>),

  SaveSession,

  LsiAction(LsiAction),
  DataManagerAction(DataManagerAction),
  ChatToolAction(ChatToolAction),

  Error(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LsiAction {
  #[serde(serialize_with = "serialize_boxed_session_action")]
  SessionAction(Box<SessionAction>),
  ChatToolResponse(Box<ChatToolAction>),
  AddWorkspace(WorkspaceParams),
  QueryWorkspaceSymbols(LsiQuery),
  GetWorkspaceFiles(LsiQuery),
  ReplaceSymbolText(String, LsiQuery),
  ReadSymbolSource(LsiQuery),
  GoToSymbolDefinition(LsiQuery),
  GoToSymbolDeclaration(LsiQuery),
  GoToTypeDefinition(LsiQuery),
  GetDiagnostics(LsiQuery),
  UpdateWorkspaceFileSymbols(PathBuf, TextDocumentIdentifier, Vec<DocumentSymbol>),
  RequestWorkspaceFileSymbols(PathBuf, TextDocumentIdentifier, usize),
  Error(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChatToolAction {
  UpdateConfig(i64, Box<SessionConfig>),
  CallTool(ChatCompletionMessageToolCall, i64),
  CompleteToolCall(String, ChatCompletionMessageToolCall, i64),
  #[serde(serialize_with = "serialize_boxed_session_action")]
  SessionAction(Box<SessionAction>),
  LsiRequest(Box<LsiAction>),
  LsiQueryResponse(String, String),
  ToolListRequest(i64),
  ToolListResponse(i64, Vec<ChatCompletionTool>),
  Error(String),
}

pub fn serialize_boxed_session_action<S>(
  action: &SessionAction,
  serializer: S,
) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  action.serialize(serializer)
}
//
// impl<'de> Deserialize<'de> for Action {
//   fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//   where
//     D: Deserializer<'de>,
//   {
//     struct ActionVisitor;
//
//     impl<'de> Visitor<'de> for ActionVisitor {
//       type Value = Action;
//
//       fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//         formatter.write_str("a valid string representation of Action")
//       }
//
//       fn visit_str<E>(self, value: &str) -> Result<Action, E>
//       where
//         E: de::Error,
//       {
//         match value {
//           "Tick" => Ok(Action::Tick),
//           "Render" => Ok(Action::Render),
//           "Suspend" => Ok(Action::Suspend),
//           "Resume" => Ok(Action::Resume),
//           "Quit" => Ok(Action::Quit),
//           "Refresh" => Ok(Action::Refresh),
//           "Help" => Ok(Action::Help),
//           "EnterInsert" => Ok(Action::EnterInsert),
//           "EnterNormal" => Ok(Action::EnterNormal),
//           data if data.starts_with("Error(") => {
//             let error_msg =
//               data.trim_start_matches("Error(").trim_end_matches(')');
//             Ok(Action::Error(error_msg.to_string()))
//           },
//           data if data.starts_with("Resize(") => {
//             let parts: Vec<&str> = data
//               .trim_start_matches("Resize(")
//               .trim_end_matches(')')
//               .split(',')
//               .collect();
//             if parts.len() == 2 {
//               let width: u16 = parts[0].trim().parse().map_err(E::custom)?;
//               let height: u16 = parts[1].trim().parse().map_err(E::custom)?;
//               Ok(Action::Resize(width, height))
//             } else {
//               Err(E::custom(format!("Invalid Resize format: {}", value)))
//             }
//           },
//           _ => Err(E::custom(format!("Unknown Action variant: {}", value))),
//         }
//       }
//     }
//
//     deserializer.deserialize_str(ActionVisitor)
//   }
// }
