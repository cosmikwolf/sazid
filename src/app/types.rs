use crate::app::consts::*;

use async_openai::{
  self,
  types::{
    ChatChoice, ChatCompletionRequestMessage, ChatCompletionResponseStreamMessage, CreateChatCompletionResponse,
    CreateChatCompletionStreamResponse, FunctionCall, FunctionCallStream, Role,
  },
};
use clap::Parser;

use bat::{
  assets::HighlightingAssets,
  config::Config,
  controller::Controller,
  style::{StyleComponent, StyleComponents},
  Input,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RenderedFunctionCall {
  pub name: String,
  pub arguments: String,
}

impl From<FunctionCallStream> for RenderedFunctionCall {
  fn from(function_call: FunctionCallStream) -> Self {
    RenderedFunctionCall {
      name: function_call.name.unwrap_or("".to_string()),
      arguments: function_call.arguments.unwrap_or("".to_string()),
    }
  }
}

impl From<FunctionCall> for RenderedFunctionCall {
  fn from(function_call: FunctionCall) -> Self {
    RenderedFunctionCall { name: function_call.name, arguments: function_call.arguments }
  }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct RenderedChatMessage {
  pub role: Option<Role>,
  pub content: Option<String>,
  pub rendered_content: Option<String>,
  pub function_call: Option<RenderedFunctionCall>,
  pub finish_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ChatResponse {
  Response(CreateChatCompletionResponse),
  StreamResponse(CreateChatCompletionStreamResponse),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ChatResponseSingleMessage {
  Response(ChatChoice),
  StreamResponse(Vec<ChatCompletionResponseStreamMessage>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ChatMessage {
  PromptMessage(ChatCompletionRequestMessage),
  ChatCompletionRequestMessage(ChatCompletionRequestMessage),
  ChatCompletionResponseMessage(ChatResponseSingleMessage),
}

impl From<&ChatMessage> for ChatCompletionRequestMessage {
  fn from(message: &ChatMessage) -> Self {
    match message {
      ChatMessage::PromptMessage(request) => request.clone(),
      ChatMessage::ChatCompletionRequestMessage(request) => request.clone(),
      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(srvec)) => {
        let content = srvec.iter().fold(Some(String::new()), |acc, stream_message| {
          acc.and_then(|mut result| {
            stream_message.delta.clone().content.map(|s| {
              result.push_str(&s);
              result
            })
          })
        });
        let function_call = srvec.iter().fold(
          Some(FunctionCall { name: "".to_string(), arguments: "".to_string() }),
          |acc, stream_message| {
            acc.and_then(|mut result| {
              stream_message.delta.clone().function_call.map(|fc| {
                if let Some(name) = fc.name {
                  result.name.push_str(name.as_str());
                }
                if let Some(arguments) = fc.arguments {
                  result.arguments.clone().push_str(arguments.as_str());
                }
                result
              })
            })
          },
        );
        ChatCompletionRequestMessage { role: Role::Assistant, content, function_call, name: None }
      },
      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::Response(response)) => {
        ChatCompletionRequestMessage {
          role: Role::Assistant,
          content: response.message.content.clone(),
          function_call: None,
          name: None,
        }
      },
    }
  }
}
impl From<&ChatMessage> for RenderedChatMessage {
  fn from(message: &ChatMessage) -> Self {
    match message {
      ChatMessage::PromptMessage(request) => RenderedChatMessage {
        role: Some(request.role.clone()),
        content: Some(format!("Prompt: {}", request.content.clone().unwrap_or("no prompt".to_string()))),
        rendered_content: None,
        function_call: None,
        finish_reason: None,
      },
      ChatMessage::ChatCompletionRequestMessage(request) => RenderedChatMessage {
        role: Some(request.role.clone()),
        content: request.content.clone(),
        rendered_content: None,
        function_call: request.function_call.clone().map(|function_call| function_call.into()),
        finish_reason: None,
      },
      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::Response(response)) => {
        RenderedChatMessage {
          role: Some(Role::Assistant),
          content: response.message.content.clone(),
          rendered_content: None,
          function_call: response.message.function_call.clone().map(|function_call| function_call.into()),
          finish_reason: response.finish_reason.clone(),
        }
      },

      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(srvec)) => {
        let finish_reason = srvec.iter().fold(Some(String::new()), |acc, stream_message| {
          acc.and_then(|mut result| {
            stream_message.finish_reason.clone().map(|s| {
              result.push_str(&s);
              result
            })
          })
        });
        let content = srvec.iter().fold(Some(String::new()), |acc, stream_message| {
          acc.and_then(|mut result| {
            stream_message.delta.content.clone().map(|s| {
              result.push_str(&s);
              result
            })
          })
        });
        let function_call = srvec.iter().fold(
          Some(FunctionCall { name: "".to_string(), arguments: "".to_string() }),
          |acc, stream_message| {
            acc.and_then(|mut result| {
              stream_message.delta.function_call.clone().map(|fc| {
                if let Some(name) = fc.name {
                  result.name.push_str(name.as_str());
                }
                if let Some(arguments) = fc.arguments {
                  result.arguments.push_str(arguments.as_str());
                }
                result
              })
            })
          },
        );
        RenderedChatMessage {
          role: Some(Role::Assistant),
          content,
          rendered_content: None,
          function_call: function_call.map(|function_call| function_call.into()),
          finish_reason,
        }
      },
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SessionData {
  pub messages: Vec<ChatMessage>,
  pub rendered_messages: Vec<RenderedChatMessage>,
  pub stylized_text: Vec<String>,
}

impl Default for SessionData {
  fn default() -> Self {
    SessionData { messages: Vec::new(), rendered_messages: Vec::new(), stylized_text: Vec::new() }
  }
}
impl From<SessionData> for String {
  fn from(session_data: SessionData) -> String {
    session_data.rendered_messages.iter().map(|m| m.into()).collect::<Vec<String>>().join("\n---\n")
  }
}
impl SessionData {
  pub fn render_new_messages(&mut self) {
    for trans_item in self.messages.iter().skip(self.rendered_messages.len()) {
      self.rendered_messages.push(RenderedChatMessage::from(trans_item));
    }
  }

  fn stylize_new_messages(&mut self) {
    let style_components = StyleComponents::new(&[
      StyleComponent::Header,
      StyleComponent::Grid,
      StyleComponent::LineNumbers,
      StyleComponent::Changes,
      StyleComponent::Rule,
      StyleComponent::Snip,
      StyleComponent::Plain,
    ]);
    let config = Config { colored_output: true, language: Some("markdown"), style_components, ..Default::default() };
    let assets = HighlightingAssets::from_binary();
    let controller = Controller::new(&config, &assets);
    &self.rendered_messages.iter().map(|mut message| {
      if message.rendered_content.is_none() || message.finish_reason.is_none() {
        let mut buffer = String::new();
        if message.rendered_content.is_none() {
          let input = Input::from_bytes(String::from(message).as_bytes());
          controller.run(vec![input.into()], Some(&mut buffer)).unwrap();
        }
        message.rendered_content = Some(buffer);
      }
    });
  }
}

impl From<&RenderedChatMessage> for String {
  fn from(message: &RenderedChatMessage) -> Self {
    let mut string_vec: Vec<String> = Vec::new();
    if let Some(content) = &message.content {
      string_vec.push(content.to_string());
    }
    if let Some(function_call) = &message.function_call {
      string_vec.push(format!("function call: {} {}", function_call.name.as_str(), function_call.arguments.as_str()));
    }
    string_vec.join("\n")
  }
}

impl From<RenderedChatMessage> for String {
  fn from(message: RenderedChatMessage) -> Self {
    let mut string_vec: Vec<String> = Vec::new();
    if let Some(content) = message.content {
      string_vec.push(content);
    }
    if let Some(function_call) = message.function_call {
      string_vec.push(format!("function call: {} {}", function_call.name.as_str(), function_call.arguments.as_str()));
    }
    string_vec.join("\n")
  }
}

// --------------------------------------
// --------------------------------------
// --------------------------------------
// --------------------------------------
// --------------------------------------
// --------------------------------------
// --------------------------------------
// --------------------------------------

// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------
// ---------------------------------------------------

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
