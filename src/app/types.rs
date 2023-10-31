use crate::{app::consts::*, trace_dbg};

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

use super::errors::ParseError;

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MessageContainer {
  pub message: ChatMessage,
  pub stylized: Option<String>,
}

impl From<ChatResponse> for Vec<ChatMessage> {
  fn from(response: ChatResponse) -> Self {
    let mut messages: Vec<ChatMessage> = Vec::new();
    match response {
      ChatResponse::Response(response) => response.choices.iter().for_each(|choice| {
        messages.push(ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::Response(choice.clone())))
      }),
      ChatResponse::StreamResponse(response) => messages
        .push(ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(response.choices))),
    }
    messages
  }
}

impl From<&ChatMessage> for ChatCompletionRequestMessage {
  fn from(message: &ChatMessage) -> Self {
    match message {
      ChatMessage::PromptMessage(request) => request.clone(),
      ChatMessage::ChatCompletionRequestMessage(request) => request.clone(),
      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(srvec)) => {
        let content = Some(
          srvec
            .iter()
            .try_fold(String::new(), |mut acc, stream_message| {
              if let Some(content) = &stream_message.delta.content {
                acc.push_str(content);
              }
              Ok::<String, ParseError>(acc)
            })
            .unwrap(),
        );
        let function_call = Some(
          srvec
            .iter()
            .try_fold(FunctionCall { name: "".to_string(), arguments: "".to_string() }, |mut acc, stream_message| {
              if let Some(function_call) = &stream_message.delta.function_call {
                if let Some(name) = &function_call.name {
                  acc.name.push_str(name.as_str());
                };
                if let Some(arguments) = &function_call.arguments {
                  acc.arguments.push_str(arguments.as_str());
                };
              }
              Ok::<FunctionCall, ParseError>(acc)
            })
            .unwrap(),
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
        let finish_reason = Some(
          srvec
            .iter()
            .try_fold(String::new(), |mut acc, stream_message| {
              if let Some(reason) = &stream_message.finish_reason {
                acc.push_str(reason);
              }
              Ok::<String, ParseError>(acc)
            })
            .unwrap(),
        );
        let content = Some(
          srvec
            .iter()
            .try_fold(String::new(), |mut acc, stream_message| {
              if let Some(content) = &stream_message.delta.content {
                acc.push_str(content);
              }
              Ok::<String, ParseError>(acc)
            })
            .unwrap(),
        );
        let function_call = Some(RenderedFunctionCall::from(
          srvec
            .iter()
            .try_fold(FunctionCall { name: "".to_string(), arguments: "".to_string() }, |mut acc, stream_message| {
              if let Some(function_call) = &stream_message.delta.function_call {
                if let Some(name) = &function_call.name {
                  acc.name.push_str(name.as_str());
                };
                if let Some(arguments) = &function_call.arguments {
                  acc.arguments.push_str(arguments.as_str());
                };
              }
              Ok::<FunctionCall, ParseError>(acc)
            })
            .unwrap(),
        ));
        RenderedChatMessage {
          role: Some(Role::Assistant),
          content,
          rendered_content: None,
          function_call,
          finish_reason,
        }
      },
    }
  }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SessionData {
  pub messages: Vec<ChatMessage>,
  pub rendered_messages: Vec<RenderedChatMessage>,
  pub stylized_text: Vec<String>,
}

impl From<SessionData> for String {
  fn from(session_data: SessionData) -> String {
    session_data.rendered_messages.iter().map(|m| m.into()).collect::<Vec<String>>().join("\n---\n")
  }
}

fn concatenate_option_strings(a: Option<String>, b: Option<String>) -> Option<String> {
  match (a, b) {
    (Some(a_str), Some(b_str)) => Some(a_str + &b_str), // Concatenate if both are Some
    (Some(a_str), None) => Some(a_str),                 // Only a is Some
    (None, Some(b_str)) => Some(b_str),                 // Only b is Some
    (None, None) => None,                               // Both are None
  }
}

fn concatenate_function_call_streams(
  call1: Option<FunctionCallStream>,
  call2: Option<FunctionCallStream>,
) -> Option<FunctionCallStream> {
  match (call1, call2) {
    (Some(fc1), Some(fc2)) => {
      Some(FunctionCallStream {
        // Choose the first `Some` or `None` if both are `None`
        name: concatenate_option_strings(fc1.name, fc2.name),
        arguments: concatenate_option_strings(fc1.arguments, fc2.arguments),
      })
    },
    (Some(fc), None) | (None, Some(fc)) => Some(fc),
    (None, None) => None,
  }
}

impl SessionData {
  pub fn add_message(&mut self, message: ChatMessage) {
    match message {
      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(new_sr_messages)) => {
        trace_dbg!("add_message: new_sr_messages: {:?}", new_sr_messages);

        if let Some(ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(
          existing_sr_messages,
        ))) = self.messages.last_mut()
        {
          // if there is an existing StreamResponse, then iterate through the Vec new_sr_messages and push any ChatCompletionResponseMessage that has the same index to it
          for new_sr_message in new_sr_messages {
            if let Some(existing_sr_message) = existing_sr_messages
              .iter_mut()
              .find(|existing_sr_message| existing_sr_message.index == new_sr_message.index)
            {
              existing_sr_message.delta.content = concatenate_option_strings(
                existing_sr_message.delta.content.clone(),
                new_sr_message.delta.content.clone(),
              );
              existing_sr_message.delta.function_call = concatenate_function_call_streams(
                existing_sr_message.delta.function_call.clone(),
                new_sr_message.delta.function_call.clone(),
              );
            } else {
              existing_sr_messages.push(new_sr_message.clone());
            }
          }
          self.render_new_messages();
        } else {
          // No existing StreamResponse, just push the message.
          self.messages.push(ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(
            new_sr_messages,
          )));
          self.render_new_messages();
        }
      },
      _ => {
        self.messages.push(message.clone());
        self.render_new_messages();
      },
    }
  }

  pub fn render_new_messages(&mut self) {
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
    self.messages.iter().for_each(|message| {
      let message = RenderedChatMessage::from(message);
      trace_dbg!("render_new_messages: message: {:?}", message.content);
      let mut buffer = String::new();
      if message.finish_reason.is_none() {
        let message_as_bytes = String::from(message);
        let input = Input::from_bytes(message_as_bytes.as_bytes());
        controller.run(vec![input.into()], Some(&mut buffer)).unwrap();
        self.stylized_text.pop();
      }
      self.stylized_text.push(buffer)
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
impl AsRef<Model> for Model {
  fn as_ref(&self) -> &Model {
    self
  }
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
