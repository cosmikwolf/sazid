use crate::app::consts::*;

use async_openai::{self, types::Role};
use clap::Parser;

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

use super::messages::RenderedChatMessage;

impl From<&RenderedChatMessage> for String {
  fn from(message: &RenderedChatMessage) -> Self {
    let mut string = String::new();
    match message.role {
      Some(Role::User) => string.push_str(&format!("You:\n{}", &message.content)),
      Some(Role::Assistant) => string.push_str(&format!("Bot:\n{}", &message.content).to_string()),
      Some(Role::Function) => {}, // string.push_str(format!("{}:\n{}", message.name.unwrap_or("".to_string()), &message.content)),
      _ => string.push_str(&message.content.to_string()),
    }
    if let Some(function_call) = &message.function_call {
      string.push_str(&format!("function call: {} {}", function_call.name.as_str(), function_call.arguments.as_str()));
    }
    string
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
  use crate::app::{
    functions::types::CommandProperty,
    helpers::{
      concatenate_function_call_streams, concatenate_option_strings, concatenate_stream_delta,
      concatenate_stream_response_messages,
    },
  };

  use super::*;
  use async_openai::types::{
    ChatCompletionResponseStreamMessage, ChatCompletionStreamResponseDelta, FinishReason, FunctionCallStream,
  };
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
