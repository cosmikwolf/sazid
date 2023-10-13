use crate::app::consts::*;
use async_openai::{
  self,
  config::OpenAIConfig,
  types::{
    ChatCompletionRequestMessage, ChatCompletionResponseMessage, ChatCompletionResponseStreamMessage,
    ChatCompletionStreamResponseDelta, FunctionCall, FunctionCallStream, Role,
  },
  Client,
};
use clap::Parser;
use color_eyre::owo_colors;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ffi::OsString, fmt::Display, path::PathBuf};

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

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChatMessage {
  pub response: Option<ChatCompletionResponseMessage>,
  pub response_stream: Option<ChatCompletionResponseStreamMessage>,
  pub request: Option<ChatCompletionRequestMessage>,
  #[serde(skip)]
  pub displayed: bool,
}

pub struct RenderedChatMessage {
  pub role: String,
  pub content: String,
  pub function_call: Option<String>,
}

// impl From<ChatMessage> for RenderedChatMessage {
//   fn from(message: ChatMessage) -> Self {
//     match message.request {
//       Some(request) => {
//         RenderedChatMessage { role: request.role, content: request.content, function_call: request.function_call }
//       },
//       None => match message.response {
//         Some(response) => RenderedChatMessage {
//         role: response.role.unwrap_or("empty".to_string()),
//           content: response.content.unwrap_or("empty".to_string()),
//           function_call: response.function_call,
//         },
//         None => RenderedChatMessage { role: String::new(), content: String::new(), function_call: None },
//       },
//       _ => RenderedChatMessage { role: String::new(), content: String::new(), function_call: None },
//     }
//   }
// }

impl Display for ChatMessage {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.request {
      Some(request) => format_chat_request(
        f,
        request.role.clone(),
        request.content.clone().unwrap(),
        request.name.clone(),
        request.function_call.clone(),
      ),
      None => match &self.response {
        Some(response) => format_chat_response(
          f,
          Some(response.role.clone()),
          response.content.clone().unwrap_or_default(),
          response.function_call.clone(),
          None,
        ),
        None => match &self.response_stream {
          Some(response_stream) => format_chat_response(
            f,
            response_stream.delta.role.clone(),
            response_stream.delta.content.clone().unwrap_or_default(),
            None,
            response_stream.delta.function_call.clone(),
          ),
          None => Ok(()),
        },
      },
    }
  }
}
impl From<ChatCompletionRequestMessage> for ChatMessage {
  fn from(request: ChatCompletionRequestMessage) -> Self {
    ChatMessage { request: Some(request), response: None, response_stream: None, displayed: false }
  }
}
impl From<ChatCompletionResponseMessage> for ChatMessage {
  fn from(response: ChatCompletionResponseMessage) -> Self {
    ChatMessage { request: None, response: Some(response), response_stream: None, displayed: false }
  }
}
impl From<ChatCompletionResponseStreamMessage> for ChatMessage {
  fn from(response_stream: ChatCompletionResponseStreamMessage) -> Self {
    ChatMessage { request: None, response: None, response_stream: Some(response_stream), displayed: false }
  }
}

impl TryFrom<ChatMessage> for ChatCompletionRequestMessage {
  type Error = &'static str;
  fn try_from(message: ChatMessage) -> Result<Self, Self::Error> {
    match message.request {
      Some(request) => Ok(ChatCompletionRequestMessage {
        role: request.role,
        content: request.content,
        name: request.name,
        function_call: request.function_call,
      }),
      None => Err("Wrong type"),
    }
  }
}
impl TryFrom<ChatMessage> for ChatCompletionResponseMessage {
  type Error = &'static str;
  fn try_from(message: ChatMessage) -> Result<Self, Self::Error> {
    match message.response {
      Some(response) => Ok(ChatCompletionResponseMessage {
        role: response.role,
        content: response.content,
        function_call: response.function_call,
      }),
      None => Err("Wrong type"),
    }
  }
}

// impl AsMut<async_openai::types::ChatCompletionRequestMessage> for ChatMessage {
//     fn as_mut(&mut self) -> &mut async_openai::types::ChatCompletionRequestMessage {
//         match self {
//             ChatMessage::ChatCompletionRequestMessage => {
//                 &mut self.as_mut()
//             }
//             _ => panic!("Wrong type"),
//         }
//     }
// }
// impl AsMut<async_openai::types::ChatCompletionResponseMessage> for ChatMessage {
//     fn as_mut(&mut self) -> &mut async_openai::types::ChatCompletionResponseMessage {
//         match self {
//             ChatMessage::ChatCompletionResponseMessage => {
//                 &mut self.as_mut()
//             }
//             _ => panic!("Wrong type"),
//         }
//     }
// }

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

fn format_chat_request(
  f: &mut std::fmt::Formatter<'_>,
  role: Role,
  message: String,
  name: Option<String>,
  function_call: Option<FunctionCall>,
) -> std::fmt::Result {
  match name {
    Some(name) => match function_call {
      Some(function_call) => {
        write!(f, "{}: {} ({:?})\n\r", role, message, (name, function_call))
      },
      None => write!(f, "{}: {} ({})\n\r", role, message, name),
    },
    None => match function_call {
      Some(function_call) => {
        write!(f, "{}: {} ({:?})\n\r", role, message, function_call)
      },
      None => write!(f, "{}: {}\n\r", role, message),
    },
  }
}

fn format_chat_response(
  f: &mut std::fmt::Formatter<'_>,
  role: Option<Role>,
  message: String,
  function_call: Option<FunctionCall>,
  function_call_stream: Option<FunctionCallStream>,
) -> std::fmt::Result {
  // todo: this shoudl aggregate the writes and return them all at once at the end

  if let Some(function_call) = function_call {
    let _ = write!(
      f,
      "{}: {:?} ({:?})\n\r",
      role.clone().unwrap(),
      message.bright_green(),
      serde_json::to_string_pretty(&function_call).unwrap().purple()
    );
  };
  if let Some(function_call_stream) = function_call_stream {
    let _ = write!(
      f,
      "{}: {:?} ({:?})\n\r",
      role.clone().unwrap(),
      message.bright_green(),
      serde_json::to_string_pretty(&function_call_stream).unwrap().purple()
    );
  };
  match role {
    Some(Role::User) => {
      write!(f, "{}: {:?}\n\r", role.unwrap(), message.bright_green())
    },
    Some(Role::Assistant) => {
      write!(f, "{}: {:?}\n\r", role.unwrap(), message.bright_blue())
    },
    Some(Role::System) => {
      write!(f, "{}: {:?}\n\r", role.unwrap(), message.bright_yellow())
    },
    Some(Role::Function) => {
      write!(f, "{}: {:?}\n\r", role.unwrap(), message.bright_yellow())
    },
    None => {
      write!(f, "{:?}\n\r", message)
    },
  }
}

fn format_chat_message(f: &mut std::fmt::Formatter<'_>, role: Role, message: String) -> std::fmt::Result {
  match role {
    Role::User => write!(f, "You: {}\n\r", message),
    Role::Assistant => write!(f, "GPT: {}\n\r", message),
    _ => Ok(()),
  }
}
