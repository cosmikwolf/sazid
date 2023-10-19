use crate::app::consts::*;
use async_openai::{
  self,
  types::{
    ChatChoice, ChatCompletionRequestMessage, ChatCompletionResponseStreamMessage, CreateChatCompletionRequest,
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse, FunctionCall, FunctionCallStream, Role,
  },
};
use clap::Parser;
use ratatui::{
  style::{Color, Style},
  text::{Line, Span},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

// options
#[derive(Parser, Clone, Default, Debug)]
#[clap(version = "1.0", author = "Tenkai Kariya", about = "Interactive chat with GPT")]
pub struct Opts {
  #[clap(short = 'n', long = "new", help = "Start a new chat session")]
  pub new: bool,

  #[clap(
    short = 'm',
    long = "model",
    value_name = "MODEL_NAME",
    help = "Specify the model to use (e.g., gpt-4, gpt-3.5-turbo-16k)"
  )]
  pub model: Option<String>,

  #[clap(short = 'b', long = "batch", help = "Respond to stdin and exit")]
  pub batch: bool,

  #[clap(short = 'f', long = "include-functions", help = "Include chat functions")]
  pub include_functions: bool,

  #[clap(short = 'l', long = "list-sessions", help = "List the models the user has access to")]
  pub list_models: bool,

  #[clap(
    short = 'p',
    long = "print-session",
    value_name = "SESSION_ID",
    default_value = "last-session",
    help = "Print a session to stdout, defaulting to the last session"
  )]
  pub print_session: String,

  #[clap(short = 's', long = "session", help = "Continue from a specified session file", value_name = "SESSION_ID")]
  pub continue_session: Option<String>,

  #[clap(short = 'i', long, value_name = "PATH", help = "Import a file or directory for GPT to process")]
  pub ingest: Option<OsString>,
}

// GPT Connector types
#[derive(Debug, Deserialize, Clone, Default)]
pub struct GPTSettings {
  pub default: Model,
  pub fallback: Model,
  pub load_session: Option<String>,
  pub save_session: Option<String>,
}

impl GPTSettings {
  fn default() -> Self {
    GPTSettings { default: GPT4.clone(), fallback: GPT3_TURBO_16K.clone(), load_session: None, save_session: None }
  }

  pub fn load(path: std::path::PathBuf) -> Self {
    match toml::from_str(std::fs::read_to_string(path).unwrap().as_str()) {
      Ok(settings) => settings,
      Err(_) => GPTSettings::default(),
    }
  }
}
#[derive(Debug, Deserialize, Clone)]
pub struct ModelConfig {
  pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Model {
  pub(crate) name: String,
  pub(crate) endpoint: String,
  pub token_limit: u32,
}

pub struct ModelsList {
  pub default: Model,
  pub fallback: Model,
}

pub struct GPTResponse {
  pub role: Role,
  pub content: String,
}

// PDF Parser types
pub struct PdfText {
  pub text: BTreeMap<u32, Vec<String>>, // Key is page number
  pub errors: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ChatTransaction {
  Request(CreateChatCompletionRequest),
  Response(CreateChatCompletionResponse),
  StreamResponse(CreateChatCompletionStreamResponse),
}

impl From<ChatTransaction> for Option<CreateChatCompletionRequest> {
  fn from(transaction: ChatTransaction) -> Self {
    match transaction {
      ChatTransaction::Request(request) => Some(request),
      _ => None,
    }
  }
}

impl From<ChatTransaction> for Option<CreateChatCompletionResponse> {
  fn from(transaction: ChatTransaction) -> Self {
    match transaction {
      ChatTransaction::Response(response) => Some(response),
      _ => None,
    }
  }
}

impl From<ChatTransaction> for Option<CreateChatCompletionStreamResponse> {
  fn from(transaction: ChatTransaction) -> Self {
    match transaction {
      ChatTransaction::StreamResponse(response) => Some(response),
      _ => None,
    }
  }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ChatMessage {
  Request(ChatCompletionRequestMessage),
  Response(ChatChoice),
  StreamResponse(ChatCompletionResponseStreamMessage),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RenderedFunctionCall {
  pub name: Option<String>,
  pub arguments: Option<String>,
}

pub struct RenderedChatMessage {
  pub id: Option<String>,
  pub role: Option<Role>,
  pub content: String,
  pub function_call: Option<RenderedFunctionCall>,
  pub finish_reason: Option<String>,
}

impl RenderedChatMessage {
  pub fn get_style(&self) -> Style {
    match self.role {
      Some(Role::User) => Style::default().fg(Color::Yellow),
      Some(Role::Assistant) => Style::default().fg(Color::Green),
      Some(Role::System) => Style::default().fg(Color::Blue),
      Some(Role::Function) => Style::default().fg(Color::Red),
      None => Style::default(),
    }
  }
}

impl<'a> From<ChatTransaction> for Vec<Line<'a>> {
  fn from(transaction: ChatTransaction) -> Self {
    let messages = <Vec<RenderedChatMessage>>::from(transaction);
    let strings: Vec<String> = messages.iter().map(|message| message.content.to_string()).collect();
    strings.join("").lines().map(|line| Line::styled(line.to_string(), messages[0].get_style())).collect()
  }
}

impl<'a> From<RenderedChatMessage> for Vec<Span<'a>> {
  fn from(value: RenderedChatMessage) -> Self {
    value.content.lines().map(|line| Span::styled(line.to_string(), value.get_style())).collect()
  }
}

impl From<FunctionCall> for RenderedFunctionCall {
  fn from(function_call: FunctionCall) -> Self {
    RenderedFunctionCall { name: Some(function_call.name), arguments: Some(function_call.arguments) }
  }
}

impl From<FunctionCallStream> for RenderedFunctionCall {
  fn from(function_call: FunctionCallStream) -> Self {
    RenderedFunctionCall { name: function_call.name, arguments: function_call.arguments }
  }
}

impl From<ChatTransaction> for Vec<RenderedChatMessage> {
  fn from(transaction: ChatTransaction) -> Self {
    match transaction {
      ChatTransaction::Request(request) => request
        .messages
        .iter()
        .map(|message| RenderedChatMessage::from(ChatMessage::Request(message.clone())))
        .collect(),
      ChatTransaction::Response(response) => response
        .choices
        .iter()
        .map(|choice| {
          let mut rendered_response = RenderedChatMessage::from(ChatMessage::Response(choice.clone()));
          rendered_response.id = Some(response.id.clone());
          rendered_response
        })
        .collect(),
      ChatTransaction::StreamResponse(response_stream) => response_stream
        .choices
        .iter()
        .map(|choice| {
          let mut rendered_response = RenderedChatMessage::from(ChatMessage::StreamResponse(choice.clone()));
          rendered_response.id = Some(response_stream.id.clone());
          rendered_response
        })
        .collect(),
    }
  }
}

impl From<ChatMessage> for RenderedChatMessage {
  fn from(message: ChatMessage) -> Self {
    match message {
      ChatMessage::Request(request) => RenderedChatMessage {
        id: None,
        role: Some(request.role),
        content: request.content.unwrap(),
        function_call: request.function_call.map(|function_call| function_call.into()),
        finish_reason: None,
      },
      ChatMessage::Response(response) => RenderedChatMessage {
        id: None,
        role: Some(response.message.role),
        content: response.message.content.unwrap(),
        function_call: response.message.function_call.map(|function_call| function_call.into()),
        finish_reason: response.finish_reason,
      },
      ChatMessage::StreamResponse(response_streams) => RenderedChatMessage {
        id: None,
        role: response_streams.delta.role,
        content: response_streams.delta.content.unwrap_or_default(),
        function_call: response_streams.delta.function_call.map(|function_call| function_call.into()),
        finish_reason: response_streams.finish_reason,
      },
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IngestedData {
  session_id: String,
  file_path: String,
  chunk_num: u32,
  content: String,
}
pub struct Message {
  pub role: Role,
  pub content: String,
}

// chunkifier types

#[allow(dead_code)]
pub struct UrlData {
  urls: String,
  data: String,
}
#[allow(dead_code)]
pub struct FilePathData {
  file_paths: String,
  data: String,
}
pub struct IngestData {
  pub text: String,
  pub urls: Vec<String>,
  pub file_paths: Vec<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CommandProperty {
  #[serde(rename = "type")]
  pub property_type: String,
  pub description: Option<String>,
  #[serde(rename = "enum", default)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub enum_values: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CommandParameters {
  #[serde(rename = "type")]
  pub param_type: String,
  pub required: Vec<String>,
  pub properties: std::collections::HashMap<String, CommandProperty>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Command {
  pub name: String,
  pub description: Option<String>,
  pub parameters: Option<CommandParameters>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Commands {
  pub commands: Vec<Command>,
}

// a display function for Message
impl std::fmt::Display for Message {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    format_chat_message(f, self.role.clone(), self.content.clone())
  }
}

fn format_chat_message(f: &mut std::fmt::Formatter<'_>, role: Role, message: String) -> std::fmt::Result {
  match role {
    Role::User => write!(f, "You: {}\n\r", message),
    Role::Assistant => write!(f, "GPT: {}\n\r", message),
    _ => Ok(()),
  }
}
