use crate::{app::consts::*, trace_dbg};

use async_openai::{
  self,
  config::OpenAIConfig,
  types::{
    CreateChatCompletionRequest,
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse, FunctionCall, FunctionCallStream, Role,
  },
};
use clap::Parser;


use serde::{Deserialize, Serialize};
use std::{
  collections::{BTreeMap},
  ffi::OsString,
  path::PathBuf,
};




#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ChatResponse {
  Response(CreateChatCompletionResponse),
  StreamResponse(CreateChatCompletionStreamResponse),
}

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
  pub function_call: Option<RenderedFunctionCall>,
  pub finish_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Transaction {
  pub id: String,
  pub request: CreateChatCompletionRequest,
  pub responses: Vec<ChatResponse>,
  pub rendered: Vec<RenderedChatMessage>,
  pub completed: bool,
  pub styled: bool,
}

use futures::StreamExt;

impl Transaction {
  pub fn new(request: CreateChatCompletionRequest) -> Self {
    let id = uuid::Uuid::new_v4().to_string();
    Transaction { id, request, responses: Vec::new(), rendered: Vec::new(), completed: false, styled: false }
  }

  pub fn new_request<R, C, E>(
    &self,
    response_callback: R,
    complete_callback: C,
    error_callback: E,
    client: async_openai::Client<OpenAIConfig>,
    stream_response: bool,
  ) where
    R: Fn(ChatResponse) + Send + 'static,
    C: Fn() + Send + 'static,
    E: Fn(String) + Send + 'static,
  {
    let request = self.request.clone();
    tokio::spawn(async move {
      match stream_response {
        true => {
          // let mut stream: Pin<Box<dyn StreamExt<Item = Result<CreateChatCompletionStreamResponse, OpenAIError>> + Send>> =
          let mut stream = client.chat().create_stream(request).await.unwrap();
          while let Some(response_result) = stream.next().await {
            match response_result {
              Ok(response) => {
                trace_dbg!("Response: {:#?}", response);
                response_callback(ChatResponse::StreamResponse(response));
              },
              Err(e) => {
                trace_dbg!("Error: {:#?} -- check https://status.openai.com/", e);
                error_callback(format!("Error: {:#?} -- check https://status.openai.com/", e));
              },
            }
          }
        },
        false => match client.chat().create(request).await {
          Ok(response) => {
            response_callback(ChatResponse::Response(response));
          },
          Err(e) => {
            trace_dbg!("Error: {}", e);
            error_callback(format!("Error: {:#?} -- check https://status.openai.com/", e));
          },
        },
      };
      complete_callback();
    });
  }

  pub fn render(&mut self) {
    if !self.completed {
      self.rendered.push(<RenderedChatMessage>::from(self.request.clone()));

      let choice_count = self
        .responses
        .iter()
        .map(|r| match r {
          ChatResponse::Response(response) => response.choices.len(),
          ChatResponse::StreamResponse(response) => response.choices.len(),
        })
        .max()
        .unwrap();
      self.rendered = vec![RenderedChatMessage::default(); choice_count];
      for (index, rendered_message) in self.rendered.iter_mut().enumerate() {
        for response in self.responses.clone() {
          match response {
            ChatResponse::Response(response) => {
              rendered_message.content = response.choices[index].message.content.clone();
              if let Some(function_call) = response.choices[index].message.function_call.clone() {
                rendered_message.function_call = Some(function_call.into())
              }
            },
            ChatResponse::StreamResponse(response) => {
              rendered_message.content = response.choices[index].delta.content.clone();
              if let Some(function_call) = response.choices[index].delta.function_call.clone() {
                match rendered_message.function_call {
                  Some(ref mut rendered_function_call) => {
                    rendered_function_call.name += function_call.name.unwrap_or("".to_string()).as_str();
                    rendered_function_call.arguments += function_call.arguments.unwrap_or("".to_string()).as_str();
                  },
                  None => rendered_message.function_call = Some(function_call.into()),
                }
              }
            },
          }
        }
      }
    }
  }
}

impl From<&Transaction> for String {
  fn from(txn: &Transaction) -> Self {
    txn
      .rendered
      .iter()
      .map(|m| {
        let mut string_vec: Vec<String> = Vec::new();
        if let Some(content) = m.content.clone() {
          string_vec.push(content);
        }
        if let Some(function_call) = m.function_call.clone() {
          string_vec.push(format!(
            "function call: {} {}",
            function_call.name.as_str(),
            function_call.arguments.as_str()
          ));
        }
        string_vec.join("\n")
      })
      .collect::<Vec<String>>()
      .join("\n")
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

impl From<CreateChatCompletionRequest> for RenderedChatMessage {
  fn from(request: CreateChatCompletionRequest) -> Self {
    RenderedChatMessage {
      role: Some(request.messages.last().unwrap().role.clone()),
      content: Some(request.messages.last().unwrap().content.clone().unwrap_or("".to_string())),
      function_call: Some(<RenderedFunctionCall>::from(
        request.messages.last().unwrap().function_call.clone().unwrap(),
      )),
      finish_reason: None,
    }
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
