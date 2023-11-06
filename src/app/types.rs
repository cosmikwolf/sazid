use crate::{app::consts::*, trace_dbg};

use async_openai::{
  self,
  types::{
    ChatChoice, ChatCompletionRequestMessage, ChatCompletionResponseStreamMessage, ChatCompletionStreamResponseDelta,
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse, FinishReason, FunctionCall, FunctionCallStream,
    Role,
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

use color_eyre::owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

use super::{errors::ParseError, markdown::render_markdown_to_string};

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
  pub name: Option<String>,
  pub finish_reason: Option<FinishReason>,
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
pub struct FunctionResult {
  pub name: String,
  pub response: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ChatMessage {
  SazidSystemMessage(String),
  FunctionResult(FunctionResult),
  PromptMessage(ChatCompletionRequestMessage),
  ChatCompletionRequestMessage(ChatCompletionRequestMessage),
  ChatCompletionResponseMessage(ChatResponseSingleMessage),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MessageContainer {
  pub message: ChatMessage,
  pub stylized: Option<String>,
  pub finished: bool,
  pub function_called: bool,
  pub response_count: usize,
}

impl MessageContainer {
  pub fn get_token_count(&self) -> Option<usize> {
    if let Some(content) = &self.stylized {
      let bpe = tiktoken_rs::cl100k_base().unwrap();
      Some(bpe.encode_with_special_tokens(content.as_str()).len())
    } else {
      None
    }
  }
}

impl From<MessageContainer> for ChatMessage {
  fn from(message_container: MessageContainer) -> Self {
    message_container.message
  }
}

impl From<&MessageContainer> for ChatMessage {
  fn from(message_container: &MessageContainer) -> Self {
    message_container.message.clone()
  }
}
impl From<ChatMessage> for MessageContainer {
  fn from(message: ChatMessage) -> Self {
    MessageContainer { message, stylized: None, finished: false, function_called: false, response_count: 0 }
  }
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

impl From<&ChatMessage> for Option<ChatCompletionRequestMessage> {
  fn from(message: &ChatMessage) -> Self {
    match message {
      ChatMessage::SazidSystemMessage(_) => None,
      ChatMessage::FunctionResult(result) => Some(ChatCompletionRequestMessage {
        role: Role::Function,
        content: Some(result.response.clone()),
        function_call: None,
        name: Some(result.name.clone()),
      }),
      ChatMessage::PromptMessage(request) => Some(request.clone()),
      ChatMessage::ChatCompletionRequestMessage(request) => Some(request.clone()),
      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(srvec)) => Some({
        let mut message = srvec[0].clone();
        srvec.iter().skip(1).for_each(|sr| {
          message = concatenate_stream_response_messages(&message, sr);
        });
        ChatCompletionRequestMessage {
          role: Role::Assistant,
          // todo: this might be a problem...
          content: Some(message.delta.content.unwrap_or("".to_string())),
          function_call: None,
          name: None,
        }
      }),
      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::Response(response)) => Some({
        ChatCompletionRequestMessage {
          role: Role::Assistant,
          content: response.message.content.clone(),
          function_call: None,
          name: None,
        }
      }),
    }
  }
}

impl AsRef<ChatMessage> for ChatMessage {
  fn as_ref(&self) -> &ChatMessage {
    self
  }
}
impl From<&ChatMessage> for RenderedChatMessage {
  fn from(message: &ChatMessage) -> Self {
    match message {
      ChatMessage::SazidSystemMessage(content) => RenderedChatMessage {
        name: None,
        role: None,
        content: Some(content.clone()),
        rendered_content: None,
        function_call: None,
        finish_reason: None,
      },
      ChatMessage::FunctionResult(result) => RenderedChatMessage {
        name: Some(result.name.clone()),
        role: Some(Role::Function),
        content: Some(result.response.clone()),
        rendered_content: None,
        function_call: None,
        finish_reason: None,
      },
      ChatMessage::PromptMessage(request) => RenderedChatMessage {
        name: None,
        role: Some(request.role),
        content: Some(format!("# Prompt\n\n*{}*", request.content.clone().unwrap_or("no prompt".to_string()))),
        rendered_content: None,
        function_call: None,
        finish_reason: None,
      },
      ChatMessage::ChatCompletionRequestMessage(request) => RenderedChatMessage {
        name: None,
        role: Some(request.role),
        content: request.content.clone(),
        rendered_content: None,
        function_call: request.function_call.clone().map(|function_call| function_call.into()),
        finish_reason: None,
      },
      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::Response(response)) => {
        RenderedChatMessage {
          name: None,
          role: Some(Role::Assistant),
          content: response.message.content.clone(),
          rendered_content: None,
          function_call: response.message.function_call.clone().map(|function_call| function_call.into()),
          finish_reason: response.finish_reason,
        }
      },
      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(srvec)) => {
        let mut message = srvec[0].clone();
        srvec.iter().skip(1).for_each(|sr| {
          message = concatenate_stream_response_messages(&message, sr);
        });
        RenderedChatMessage {
          name: None,
          role: Some(Role::Assistant),
          content: message.delta.content,
          rendered_content: None,
          function_call: message.delta.function_call.map(|function_call| function_call.into()),
          finish_reason: message.finish_reason,
        }
      },
    }
  }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SessionData {
  pub messages: Vec<MessageContainer>,
  pub stylized_lines: Vec<String>,
  pub stylized_text: String,
  pub unrendered_text: String,
}

impl From<SessionData> for String {
  fn from(session_data: SessionData) -> String {
    session_data
      .messages
      .iter()
      .map(|m| String::from(RenderedChatMessage::from(m.message.as_ref())))
      .collect::<Vec<String>>()
      .join("\n")
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

fn concatenate_stream_delta(
  delta1: ChatCompletionStreamResponseDelta,
  delta2: ChatCompletionStreamResponseDelta,
) -> ChatCompletionStreamResponseDelta {
  ChatCompletionStreamResponseDelta {
    role: delta1.role,
    content: concatenate_option_strings(delta1.content, delta2.content),
    function_call: concatenate_function_call_streams(delta1.function_call, delta2.function_call),
  }
}
fn concatenate_finish_reason(
  finish_reason1: Option<FinishReason>,
  finish_reason2: Option<FinishReason>,
) -> Result<Option<FinishReason>, ParseError> {
  match (finish_reason1, finish_reason2) {
    (Some(_), Some(_)) => Err(ParseError::new("Cannot concatenate two finish reasons")),
    (Some(fr), None) => Ok(Some(fr)),
    (None, Some(fr)) => Ok(Some(fr)),
    (None, None) => Ok(None), // todo: handle this case
  }
}
fn concatenate_stream_response_messages(
  sr1: &ChatCompletionResponseStreamMessage,
  sr2: &ChatCompletionResponseStreamMessage,
) -> ChatCompletionResponseStreamMessage {
  ChatCompletionResponseStreamMessage {
    index: sr1.index,
    delta: concatenate_stream_delta(sr1.delta.clone(), sr2.delta.clone()),
    finish_reason: concatenate_finish_reason(sr1.finish_reason, sr2.finish_reason).unwrap(),
  }
}

fn collate_stream_response_vec(
  new_srvec: Vec<ChatCompletionResponseStreamMessage>,
  existing_srvec: &mut Vec<ChatCompletionResponseStreamMessage>,
) {
  // trace_dbg!("add_message: supplimental delta \n{:?}\n{:?}", new_srvec, existing_srvec);
  new_srvec.iter().for_each(|new_sr| {
    if !existing_srvec.iter_mut().any(|existing_sr| {
      if existing_sr.index == new_sr.index {
        *existing_sr = concatenate_stream_response_messages(existing_sr, new_sr);
        true
      } else {
        false
      }
    }) {
      existing_srvec.push(new_sr.clone());
    }
  });
}
impl SessionData {
  pub fn add_message(&mut self, message: ChatMessage) {
    match message {
      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(new_srvec)) => {
        if let Some(mc) = self.messages.last_mut() {
          if mc.finished {
            self.messages.push(
              ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(new_srvec)).into(),
            );
          } else if let ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(
            existing_srvec,
          )) = &mut mc.message
          {
            collate_stream_response_vec(new_srvec, existing_srvec);
          } else {
            trace_dbg!("add_message: new delta {:?}", new_srvec);
            // No existing StreamResponse, just push the message.
            self.messages.push(
              ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(new_srvec)).into(),
            );
          }
        } else {
          self.messages.push(
            ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(new_srvec)).into(),
          );
        }
      },
      _ => {
        self.messages.push(message.clone().into());
      },
    };
    // return a vec of any functions that need to be called
    self.post_process_new_messages()
  }
  pub fn post_process_new_messages(&mut self) {
    for message in self.messages.iter_mut().filter(|m| !m.finished) {
      SessionData::render_message_pulldown_cmark(message, true);
      let rendered_message = RenderedChatMessage::from(&message.message);
      if rendered_message.finish_reason.is_some() {
        message.finished = true;
        trace_dbg!("post_process_new_messages: finished message {:#?}", message);
      }
    }
    self.stylized_lines = self.stylized_text();
    self.stylized_text = self.stylized_lines.join("\n");
    //self.unrendered_text = self.stylized_text[stylized_text_char_len..].to_string();
  }

  pub fn get_functions_that_need_calling(&mut self) -> Vec<RenderedFunctionCall> {
    self
      .messages
      .iter_mut()
      .filter(|m| m.finished && !m.function_called)
      .filter_map(|m| {
        m.function_called = true;
        RenderedChatMessage::from(&m.message).function_call
      })
      .collect()
  }

  fn render_message_pulldown_cmark(message: &mut MessageContainer, format_responses_only: bool) {
    let rendered_message = String::from(RenderedChatMessage::from(&ChatMessage::from(message.clone())));
    if format_responses_only {
      if matches!(message.message, ChatMessage::ChatCompletionResponseMessage(_)) {
        message.stylized = Some(render_markdown_to_string(rendered_message))
      } else {
        message.stylized = Some(rendered_message)
      }
    } else {
      message.stylized = Some(render_markdown_to_string(rendered_message))
    }
  }

  fn _render_message(message: &mut MessageContainer) {
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
    let rendered_message = RenderedChatMessage::from(&ChatMessage::from(message.clone()));
    let stylize_option = false;
    let message_content = String::from(rendered_message);
    message.stylized = if stylize_option {
      let mut buffer = String::new();
      let input = Input::from_bytes(message_content.as_bytes());
      controller.run(vec![input.into()], Some(&mut buffer)).unwrap();
      Some(buffer)
    } else {
      Some(message_content)
    }
  }

  pub fn stylized_text(&self) -> Vec<String> {
    self
      .messages
      .iter()
      .flat_map(|m| m.stylized.clone())
      .collect::<Vec<String>>()
      .join("\n")
      .lines()
      .map(|l| l.to_string())
      .collect()
  }
}

impl From<RenderedChatMessage> for String {
  fn from(message: RenderedChatMessage) -> Self {
    let mut string_vec: Vec<String> = Vec::new();
    if let Some(content) = message.content {
      match message.role {
        Some(Role::User) => string_vec.push(format!("***You:*** {}", content)),
        Some(Role::Assistant) => string_vec.push(format!("***Bot:*** {}", content).bright_red().to_string()),
        Some(Role::Function) => string_vec.push(format!("{}: {}", message.name.unwrap_or("".to_string()), content)),
        _ => string_vec.push(content),
      }
    }
    if let Some(function_call) = message.function_call {
      string_vec.push(format!("function call: {} {}", function_call.name.as_str(), function_call.arguments.as_str()));
    }
    string_vec.join("\n")
  }
}

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

// a display function for Message
impl std::fmt::Display for Message {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    format_chat_message(f, self.role, self.content.clone())
  }
}

fn format_chat_message(f: &mut std::fmt::Formatter<'_>, role: Role, message: String) -> std::fmt::Result {
  match role {
    Role::User => write!(f, "You: {}\n\r", message),
    Role::Assistant => write!(f, "GPT: {}\n\r", message),
    _ => Ok(()),
  }
}

#[cfg(test)]
mod tests {
  use crate::app::llm_functions::types::CommandProperty;

  use super::*;
  use serde_json::to_string;

  #[test]
  fn test_serialization_command_properties() {
    // Manually construct the expected `CommandProperty` vector
    let location_property = CommandProperty {
      name: "location".to_owned(),
      required: true,
      property_type: "string".to_owned(),
      description: Some("The city and state, e.g. San Francisco, CA".to_owned()),
      enum_values: None,
    };
    let unit_property = CommandProperty {
      name: "unit".to_owned(),
      required: false,
      property_type: "string".to_owned(),
      description: None,
      enum_values: Some(vec!["celsius".to_owned(), "fahrenheit".to_owned()]),
    };
    let properties_vec = vec![location_property, unit_property];

    // Serialize the vector into JSON
    let serialized_properties = to_string(&properties_vec).expect("Failed to serialize properties");

    // Since the serialization will not include the `name` and `required` fields (due to `#[serde(skip)]`),
    // we need to adjust the expected JSON to match this format.
    let expected_json = r#"[
            {
                "type": "string",
                "description": "The city and state, e.g. San Francisco, CA",
                "enum": null
            },
            {
                "type": "string",
                "description": null,
                "enum": ["celsius", "fahrenheit"]
            }
        ]"#;

    assert_eq!(serialized_properties, expected_json);
  }

  // Concatenate Function implementations (concatenate_option_strings, concatenate_function_call_streams, etc.)

  #[test]
  fn test_concatenate_option_strings() {
    assert_eq!(
      concatenate_option_strings(Some("Hello".to_string()), Some(" world!".to_string())),
      Some("Hello world!".to_string())
    );
    assert_eq!(concatenate_option_strings(Some("Hello".to_string()), None), Some("Hello".to_string()));
    assert_eq!(concatenate_option_strings(None, Some("world!".to_string())), Some("world!".to_string()));
    assert_eq!(concatenate_option_strings(None::<String>, None::<String>), None);
  }

  #[test]
  fn test_concatenate_function_call_streams() {
    let fc1 = FunctionCallStream { name: Some("func1".to_string()), arguments: Some("arg1".to_string()) };
    let fc2 = FunctionCallStream { name: Some("func2".to_string()), arguments: Some("arg2".to_string()) };
    assert_eq!(
      concatenate_function_call_streams(Some(fc1.clone()), Some(fc2.clone())),
      Some(FunctionCallStream { name: Some("func1func2".to_string()), arguments: Some("arg1arg2".to_string()) })
    );
    assert_eq!(concatenate_function_call_streams(Some(fc1.clone()), None), Some(fc1.clone()));
    assert_eq!(concatenate_function_call_streams(None, Some(fc2.clone())), Some(fc2.clone()));
    assert_eq!(concatenate_function_call_streams(None::<FunctionCallStream>, None::<FunctionCallStream>), None);
  }

  #[test]
  fn test_concatenate_stream_delta() {
    let delta1 = ChatCompletionStreamResponseDelta {
      role: Some(Role::User),
      content: Some("hello".to_string()),
      function_call: Some(FunctionCallStream { name: Some("greet".to_string()), arguments: Some("".to_string()) }),
    };
    let delta2 = ChatCompletionStreamResponseDelta {
      role: Some(Role::Assistant),
      content: Some(" world".to_string()),
      function_call: Some(FunctionCallStream { name: Some("response".to_string()), arguments: Some("".to_string()) }),
    };
    assert_eq!(
      concatenate_stream_delta(delta1, delta2),
      ChatCompletionStreamResponseDelta {
        role: Some(Role::User), // The role is taken from the first delta
        content: Some("hello world".to_string()),
        function_call: Some(FunctionCallStream {
          name: Some("greetresponse".to_string()),
          arguments: Some("".to_string())
        }),
      }
    );
  }

  #[test]
  fn test_concatenate_stream_response_messages() {
    let sr1 = ChatCompletionResponseStreamMessage {
      index: 1,
      delta: ChatCompletionStreamResponseDelta {
        role: Some(Role::User),
        content: Some("hello".to_string()),
        function_call: Some(FunctionCallStream { name: Some("greet".to_string()), arguments: Some("".to_string()) }),
      },
      finish_reason: None,
    };
    let sr2 = ChatCompletionResponseStreamMessage {
      index: 2, // Index is different, but concatenate_stream_response_messages uses sr1's index
      delta: ChatCompletionStreamResponseDelta {
        role: Some(Role::Assistant),
        content: Some(" world".to_string()),
        function_call: Some(FunctionCallStream { name: Some("response".to_string()), arguments: Some("".to_string()) }),
      },
      finish_reason: Some(FinishReason::Stop), // This is ignored in concatenate_stream_response_messages
    };
    assert_eq!(
      concatenate_stream_response_messages(&sr1, &sr2),
      ChatCompletionResponseStreamMessage {
        index: 1, // The index from sr1 is used
        delta: ChatCompletionStreamResponseDelta {
          role: Some(Role::User),
          content: Some("hello world".to_string()),
          function_call: Some(FunctionCallStream {
            name: Some("greetresponse".to_string()),
            arguments: Some("".to_string())
          }),
        },
        finish_reason: Some(FinishReason::Stop), // The finish_reason from sr1 is used
      }
    );
  }
}
