use crate::{app::consts::*, trace_dbg};
use ansi_to_tui::IntoText;
use async_openai::{
  self,
  types::{
    ChatChoice, ChatCompletionRequestMessage, ChatCompletionResponseStreamMessage, CreateChatCompletionRequest,
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse, FunctionCall, FunctionCallStream, Role,
  },
};
use clap::Parser;
use nu_ansi_term::Color;
use ratatui::text::Text;
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
  pub txn_id: String,
  pub originals: Vec<ChatTransaction>,
  pub rendered: Option<Vec<RenderedChatTransaction>>,
  pub completed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ChatTransaction {
  Request(CreateChatCompletionRequest),
  Response(CreateChatCompletionResponse),
  StreamResponse(CreateChatCompletionStreamResponse),
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RenderedChatMessage {
  pub role: Option<Role>,
  pub content: Option<String>,
  pub function_call: Option<RenderedFunctionCall>,
  pub finish_reason: Option<String>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct RenderedChatTransaction {
  pub id: Option<String>,
  pub choices: Vec<RenderedChatMessage>,
}

impl<'a> From<ChatTransaction> for Text<'a> {
  fn from(transaction: ChatTransaction) -> Self {
    let string = <String>::from(transaction);
    string.bytes().collect::<Vec<u8>>().into_text().unwrap()
  }
}
impl ChatTransaction {
  pub fn get_rendered_chat_messages(&self) {
    match self {
      ChatTransaction::StreamResponse(sr) => {},
      ChatTransaction::Response(response) => {},
      ChatTransaction::Request(request) => {},
    }
  }
}

impl From<ChatTransaction> for Vec<RenderedChatMessage> {
  fn from(transaction: ChatTransaction) -> Self {
    match transaction {
      ChatTransaction::Request(request) => vec![request
        .messages
        .iter()
        .map(|message| RenderedChatMessage::from(ChatMessage::Request(message.clone())))
        .last()
        .unwrap()],
      ChatTransaction::Response(response) => {
        response.choices.iter().map(|choice| RenderedChatMessage::from(ChatMessage::Response(choice.clone()))).collect()
      },
      ChatTransaction::StreamResponse(response_stream) => response_stream
        .choices
        .iter()
        .map(|choice| RenderedChatMessage::from(ChatMessage::StreamResponse(choice.clone())))
        .collect(),
    }
  }
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

use bat::{assets::HighlightingAssets, config::Config, controller::Controller, Input};

impl From<ChatTransaction> for String {
  fn from(transaction: ChatTransaction) -> Self {
    let get_content = |messages: Vec<RenderedChatMessage>| {
      messages
        .iter()
        .map(|message| message.content.clone().unwrap().to_string())
        .collect::<Vec<String>>()
        .join("")
        .lines()
        .map(|line| line.to_string())
        .collect::<Vec<String>>()
        .join("\n")
    };

    let consolidate_stream_fragments = |messages: Vec<RenderedChatMessage>| {
      let mut consolidated_messages: Vec<RenderedChatMessage> = Vec::new();
      let mut consolidated_message = RenderedChatMessage::default();
      let mut consolidated_function_call_names: Vec<Option<String>> = Vec::new();
      let mut consolidated_function_call_arguments: Vec<Option<String>> = Vec::new();
      let mut it = messages.iter().peekable();
      while let Some(message) = it.next() {
        if message.role.is_some() {
          consolidated_message.role = message.role.clone();
        }
        if message.function_call.is_some() {
          consolidated_function_call_names.push(message.function_call.as_ref().unwrap().name.clone());
          consolidated_function_call_arguments.push(message.function_call.as_ref().unwrap().arguments.clone());
        }
        match consolidated_message.content {
          Some(_) => {
            consolidated_message.content = Some(format!(
              "{}{}",
              consolidated_message.content.unwrap(),
              message.content.clone().unwrap_or("".to_string())
            ))
          },
          None => consolidated_message.content = message.content.clone(),
        }
        if message.finish_reason.is_some() || it.peek().is_none() {
          if !consolidated_function_call_names.is_empty() || !consolidated_function_call_arguments.is_empty() {
            consolidated_message.function_call = Some(RenderedFunctionCall {
              name: Some(
                consolidated_function_call_names.clone().into_iter().flatten().collect::<Vec<String>>().join(" "),
              ),
              arguments: Some(
                consolidated_function_call_arguments.clone().into_iter().flatten().collect::<Vec<String>>().join(" "),
              ),
            });
          }
          consolidated_function_call_arguments = Vec::new();
          consolidated_function_call_names = Vec::new();
          consolidated_message.finish_reason = message.finish_reason.clone();
          consolidated_messages.push(consolidated_message.clone());
          consolidated_message = RenderedChatMessage::default();
        }
      }
      consolidated_messages
    };

    match transaction {
      ChatTransaction::Request(request) => {
        let content = get_content(vec![request
          .messages
          .iter()
          .map(|message| RenderedChatMessage::from(ChatMessage::Request(message.clone())))
          .last()
          .unwrap()]);
        Color::Magenta.paint(content).to_string()
      },
      ChatTransaction::Response(response) => {
        let content = get_content(
          response
            .choices
            .iter()
            .map(|choice| RenderedChatMessage::from(ChatMessage::Response(choice.clone())))
            .collect(),
        );
        Color::Cyan.paint(content).to_string()
      },
      ChatTransaction::StreamResponse(response_stream) => {
        let messages = consolidate_stream_fragments(
          response_stream
            .choices
            .iter()
            .map(|choice| RenderedChatMessage::from(ChatMessage::StreamResponse(choice.clone())))
            .collect(),
        );
        let config = Config { colored_output: true, language: Some("markdown"), ..Default::default() };
        let assets = HighlightingAssets::from_binary();
        let controller = Controller::new(&config, &assets);
        let mut buffer = String::new();
        for message in messages {
          let mut text = String::new();
          if let Some(content) = message.content {
            text += format!("{}\n", content).as_str()
          }
          if let Some(function_call) = message.function_call {
            text += format!(
              "executing function: {} {}\n",
              function_call.name.unwrap_or("none".to_string()),
              function_call.arguments.unwrap_or("none".to_string())
            )
            .as_str()
          }
          let input = Input::from_bytes(text.as_bytes());
          controller.run(vec![input.into()], Some(&mut buffer)).unwrap();
        }
        buffer
      },
    }
  }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelConfig {
  pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct Model {
  pub name: String,
  pub endpoint: String,
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

// impl<'a> From<RenderedChatMessage> for Vec<Span<'a>> {
//   fn from(value: RenderedChatMessage) -> Self {
//     value.content.lines().map(|line| Span::styled(line.to_string(), value.get_style())).collect()
//   }
// }

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

impl From<ChatMessage> for RenderedChatMessage {
  fn from(message: ChatMessage) -> Self {
    match message {
      ChatMessage::Request(request) => RenderedChatMessage {
        role: Some(request.role),
        content: request.content,
        function_call: request.function_call.map(|function_call| function_call.into()),
        finish_reason: None,
      },
      ChatMessage::Response(response) => RenderedChatMessage {
        role: Some(response.message.role),
        content: response.message.content,
        function_call: response.message.function_call.map(|function_call| function_call.into()),
        finish_reason: response.finish_reason,
      },
      ChatMessage::StreamResponse(response_streams) => RenderedChatMessage {
        role: response_streams.delta.role,
        content: response_streams.delta.content,
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
