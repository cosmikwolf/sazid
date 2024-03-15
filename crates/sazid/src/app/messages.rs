use bitflags::bitflags;
use std::{
  collections::HashSet,
  fmt::{self, Formatter},
  sync::Arc,
};

use color_eyre::owo_colors::OwoColorize;
use helix_core::syntax::Loader;
use ropey::Rope;
use serde::{Deserialize, Serialize};

use async_openai::{
  self,
  types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
    ChatCompletionRequestFunctionMessage, ChatCompletionRequestMessage,
    ChatCompletionRequestMessageContentPart,
    ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
    ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse,
    FunctionCall, FunctionCallStream, Role,
  },
};

use crate::trace_dbg;

use super::{
  errors::ParseError,
  helpers::{
    get_assistant_message_from_create_chat_completion_response,
    get_assistant_message_from_create_chat_completion_stream_response,
  },
  // markdown::Markdown,
};

bitflags! {

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MessageState:u8 {
const RECEIVING = 1 << 0;
const RECEIVE_COMPLETE = 1<< 1;
const TEXT_RENDERED = 1 << 2;
const TOOLS_COMPLETE = 1 << 3;
const EMBEDDING_SAVED = 1 << 4;
const IS_CURRENT_TRANSACTION = 1 << 5;
const HAS_UNRENDERED_CONTENT = 1 << 6;
}

}

impl MessageContainer {
  pub fn is_receiving(&self) -> bool {
    self.message_state.contains(MessageState::RECEIVING)
  }

  pub fn set_receive_complete(&mut self) {
    self.message_state.set(MessageState::RECEIVE_COMPLETE, true);
    self.message_state.set(MessageState::RECEIVING, false);
  }

  pub fn set_has_unrendered_content(&mut self) {
    self.message_state.set(MessageState::HAS_UNRENDERED_CONTENT, true);
  }

  pub fn unset_has_unrendered_content(&mut self) {
    self.message_state.set(MessageState::HAS_UNRENDERED_CONTENT, false);
  }

  pub fn has_unrendered_content(&self) -> bool {
    self.message_state.contains(MessageState::HAS_UNRENDERED_CONTENT)
  }

  pub fn set_text_rendered(&mut self) {
    self.message_state.set(MessageState::TEXT_RENDERED, true);
  }
  pub fn set_tools_complete(&mut self) {
    self.message_state.set(MessageState::TOOLS_COMPLETE, true);
  }
  pub fn is_receive_complete(&self) -> bool {
    self.message_state.contains(MessageState::RECEIVE_COMPLETE)
  }

  pub fn is_complete(&self) -> bool {
    self.message_state.contains(MessageState::RECEIVE_COMPLETE)
      && self.message_state.contains(MessageState::TEXT_RENDERED)
      && self.message_state.contains(MessageState::TOOLS_COMPLETE)
      && self.message_state.contains(MessageState::EMBEDDING_SAVED)
  }

  pub fn set_current_transaction_flag(&mut self) {
    self.message_state.set(MessageState::IS_CURRENT_TRANSACTION, true);
  }

  pub fn is_current_transaction(&self) -> bool {
    self.message_state.contains(MessageState::IS_CURRENT_TRANSACTION)
  }

  pub fn vertical_height(
    &self,
    window_width: usize,
    lang_config: Arc<Loader>,
  ) -> usize {
    let content = format!("{}", self);
    // let markdown = Markdown::new(content, window_width, lang_config);
    //
    // let text = markdown.parse(None);
    // text.len()
    0
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MessageContainer {
  pub message: ChatCompletionRequestMessage,
  pub receive_buffer: Option<ReceiveBuffer>,
  pub tool_calls: Vec<ChatCompletionMessageToolCall>,
  pub message_id: i64,
  pub stream_id: Option<String>,
  pub selected_choice: usize,
  pub tools_called: bool,
  pub receive_complete: bool,
  pub embedding_saved: bool,
  pub current_transaction_flag: bool,
  pub stylize_complete: bool,
  pub response_count: usize,
  pub wrapped_content: String,
  #[serde(skip)]
  pub stylized: Rope,
  pub token_usage: usize,
  #[serde(skip)]
  pub rendered_line_count: usize,
  pub message_state: MessageState,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ChatMessage {
  Response(CreateChatCompletionResponse),
  StreamResponse(Vec<CreateChatCompletionStreamResponse>),
  System(ChatCompletionRequestSystemMessage),
  User(ChatCompletionRequestUserMessage),
  Assistant(ChatCompletionRequestAssistantMessage),
  Tool(ChatCompletionRequestToolMessage),
  Function(ChatCompletionRequestFunctionMessage),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ReceiveBuffer {
  Response(CreateChatCompletionResponse),
  StreamResponse(Vec<CreateChatCompletionStreamResponse>),
}

impl From<ReceiveBuffer> for ChatCompletionRequestMessage {
  fn from(buffer: ReceiveBuffer) -> Self {
    match buffer {
      ReceiveBuffer::Response(response) => {
        ChatCompletionRequestMessage::Assistant(
          get_assistant_message_from_create_chat_completion_response(
            0, &response,
          )
          .unwrap(),
        )
      },
      ReceiveBuffer::StreamResponse(response) => {
        ChatCompletionRequestMessage::Assistant(
          get_assistant_message_from_create_chat_completion_stream_response(
            0, &response,
          )
          .unwrap(),
        )
      },
    }
  }
}

impl From<ChatMessage> for ChatCompletionRequestMessage {
  fn from(message: ChatMessage) -> Self {
    message.into()
  }
}

impl From<ReceiveBuffer> for MessageContainer {
  fn from(receive_buffer: ReceiveBuffer) -> Self {
    let message = receive_buffer.clone().into();
    let (message_state, stream_id) = match &receive_buffer {
      ReceiveBuffer::StreamResponse(srvec) => {
        (MessageState::RECEIVING, Some(srvec[0].id.clone()))
      },
      _ => (MessageState::RECEIVE_COMPLETE, None),
    };
    MessageContainer {
      selected_choice: 0,
      receive_buffer: Some(receive_buffer),
      message_id: rand::random::<i64>(),
      message,
      stream_id,
      receive_complete: false,
      tool_calls: Vec::new(),
      wrapped_content: String::new(),
      stylized: Rope::new(),
      tools_called: false,
      response_count: 0,
      token_usage: 0,
      embedding_saved: false,
      stylize_complete: false,
      current_transaction_flag: false,
      message_state,
      rendered_line_count: 0,
    }
  }
}

impl From<ChatMessage> for MessageContainer {
  fn from(message: ChatMessage) -> Self {
    match message {
      ChatMessage::Response(response) => {
        ReceiveBuffer::Response(response).into()
      },
      ChatMessage::StreamResponse(response) => {
        ReceiveBuffer::StreamResponse(response).into()
      },
      ChatMessage::Tool(message) => {
        MessageContainer::new_from_completed_message(
          ChatCompletionRequestMessage::Tool(message),
        )
      },
      ChatMessage::Function(message) => {
        MessageContainer::new_from_completed_message(
          ChatCompletionRequestMessage::Function(message),
        )
      },
      ChatMessage::System(message) => {
        MessageContainer::new_from_completed_message(
          ChatCompletionRequestMessage::System(message),
        )
      },
      ChatMessage::User(message) => {
        MessageContainer::new_from_completed_message(
          ChatCompletionRequestMessage::User(message),
        )
      },
      ChatMessage::Assistant(message) => {
        MessageContainer::new_from_completed_message(
          ChatCompletionRequestMessage::Assistant(message),
        )
      },
    }
  }
}

pub fn chat_completion_request_message_as_str(
  message: &ChatCompletionRequestMessage,
) -> &str {
  match &message {
    ChatCompletionRequestMessage::System(system_message) => {
      &system_message.content
    },
    ChatCompletionRequestMessage::User(user_message) => {
      match &user_message.content {
        ChatCompletionRequestUserMessageContent::Text(text) => text,
        ChatCompletionRequestUserMessageContent::Array(parts) => parts
          .iter()
          .map(|part| match part {
            ChatCompletionRequestMessageContentPart::Text(text) => {
              text.text.as_str()
            },
            ChatCompletionRequestMessageContentPart::Image(image) => {
              image.image_url.url.as_str()
            },
          })
          .next()
          .unwrap_or(""),
      }
    },
    ChatCompletionRequestMessage::Assistant(assistant_message) => {
      match &assistant_message.content {
        Some(content) => content,
        None => "",
      }
    },
    ChatCompletionRequestMessage::Tool(tool_message) => &tool_message.content,
    ChatCompletionRequestMessage::Function(function_message) => {
      match &function_message.content {
        Some(content) => content,
        None => "",
      }
    },
  }
}

pub fn get_chat_message_text(message: &ChatCompletionRequestMessage) -> String {
  match message {
    ChatCompletionRequestMessage::System(message) => {
      message.content.to_string()
    },
    ChatCompletionRequestMessage::User(message) => match &message.content {
      ChatCompletionRequestUserMessageContent::Text(content) => content.clone(),
      ChatCompletionRequestUserMessageContent::Array(parts) => {
        let mut content: Vec<String> = Vec::new();
        for part in parts {
          content.push(match part {
            ChatCompletionRequestMessageContentPart::Text(content) => {
              content.text.clone()
            },
            ChatCompletionRequestMessageContentPart::Image(content) => {
              content.image_url.url.clone()
            },
          })
        }
        content.join("\n")
      },
    },
    ChatCompletionRequestMessage::Assistant(message) => {
      let mut content: Vec<String> = Vec::new();
      content.push(match &message.content {
        Some(content) => content.clone(),
        None => "no content".to_string(),
      });
      match &message.tool_calls {
        Some(tool_calls) => {
          for tool_call in tool_calls {
            content.push(tool_call.function.name.clone());
            content.push(tool_call.function.arguments.clone());
          }
        },
        None => {},
      }
      content.join(" ")
    },
    ChatCompletionRequestMessage::Tool(message) => {
      let content =
        vec![message.tool_call_id.clone(), message.content.to_string()];
      content.join(" ")
    },
    ChatCompletionRequestMessage::Function(message) => {
      let content = vec![
        message.name.clone(),
        match &message.content {
          Some(content) => content.to_string(),
          None => "no function content".to_string(),
        },
      ];
      content.join(" ")
    },
  }
}

impl fmt::Display for MessageContainer {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", get_chat_message_text(&self.message))
  }
}

impl MessageContainer {
  fn new(message: ChatCompletionRequestMessage) -> Self {
    MessageContainer {
      message,
      receive_buffer: None,
      tool_calls: Vec::new(),
      message_id: rand::random::<i64>(),
      stream_id: None,
      selected_choice: 0,
      embedding_saved: false,
      stylize_complete: false,
      receive_complete: false,
      current_transaction_flag: false,
      wrapped_content: String::new(),
      stylized: Rope::new(),
      tools_called: false,
      response_count: 0,
      token_usage: 0,
      message_state: MessageState::empty(),
      rendered_line_count: 0,
    }
  }

  pub fn new_from_completed_message(
    message: ChatCompletionRequestMessage,
  ) -> Self {
    let mut message_container = MessageContainer::new(message);
    message_container.message_state = MessageState::RECEIVE_COMPLETE;
    message_container
  }

  // pub fn new_from_receive_buffer(receive_buffer: ReceiveBuffer) -> Self {
  //   match &receive_buffer {
  //     ReceiveBuffer::Response(response) => {
  //       let mut message = MessageContainer::new(ChatCompletionRequestMessage::Assistant(
  //         get_assistant_message_from_create_chat_completion_response(0, response).unwrap(),
  //       ));
  //       message.receive_buffer = Some(receive_buffer.clone());
  //       message
  //     },
  //     ReceiveBuffer::StreamResponse(response) => {
  //       let mut message = MessageContainer::new(ChatCompletionRequestMessage::Assistant(
  //         get_assistant_message_from_create_chat_completion_stream_response(0, response).unwrap(),
  //       ));
  //       message.receive_buffer = Some(receive_buffer.clone());
  //       message.stream_id = Some(response[0].id.clone());
  //       message
  //     },
  //   }
  // }

  pub fn update_stream_response(
    &mut self,
    stream_message: CreateChatCompletionStreamResponse,
  ) -> Result<(), ParseError> {
    if self.stream_id == Some(stream_message.id.clone()) {
      match &mut self.receive_buffer {
        Some(ReceiveBuffer::StreamResponse(srvec)) => {

          srvec.push(stream_message);

          self.message = ChatCompletionRequestMessage::Assistant(
            get_assistant_message_from_create_chat_completion_stream_response(self.selected_choice, srvec).unwrap(),
          );
          self.check_if_receive_is_complete();
          Ok(())
        },
        _ => Err(ParseError::new("MessageContainer::update_stream_response: message is not a stream response")),
      }
    } else {
      Err(ParseError::new(
        "MessageContainer::update_stream_response: stream id does not match",
      ))
    }
  }

  pub fn check_if_receive_is_complete(&mut self) {
    if match &self.receive_buffer {
      Some(ReceiveBuffer::Response(response)) => {
        response.choices.iter().all(|c| c.finish_reason.is_some())
      },
      Some(ReceiveBuffer::StreamResponse(srvec)) => {
        let mut indexes_with_finish_reason = HashSet::new();

        // First, insert indices that have a finish_reason into the set.
        srvec.iter().for_each(|response| {
          response.choices.iter().for_each(|choice| {
            if choice.finish_reason.is_some() {
              indexes_with_finish_reason.insert(choice.index);
            }
          });
        });

        // Now, check if every index has a corresponding finish_reason.
        let res = srvec.iter().all(|response| {
          response
            .choices
            .iter()
            .all(|choice| indexes_with_finish_reason.contains(&choice.index))
        });
        if res {
          trace_dbg!(
            "message finished: {:#?}",
            self.stream_id.bright_magenta()
          );
        } else {
          trace_dbg!(
            "message not finished: {:#?}",
            self.stream_id.bright_magenta()
          );
        }
        res
      },
      _ => true,
    } {
      self.set_receive_complete();
    }
  }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct RenderedChatMessage {
  pub role: Option<Role>,
  pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FunctionResult {
  pub name: String,
  pub response: String,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq)]
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
    RenderedFunctionCall {
      name: function_call.name,
      arguments: function_call.arguments,
    }
  }
}

// impl From<MessageContainer> for ChatMessage {
//   fn from(message_container: MessageContainer) -> Self {
//     message_container.message
//   }
// }
//
// impl From<&MessageContainer> for ChatMessage {
//   fn from(message_container: &MessageContainer) -> Self {
//     message_container.message.clone()
//   }
// }
//
// impl From<ChatMessage> for MessageContainer {
//   fn from(message: ChatMessage) -> Self {
//     let stream_id = match message {
//       ChatMessage::StreamResponse(srvec) => Some(srvec[0].id.clone()),
//       _ => None,
//     };
//     MessageContainer {
//       selected_choice: 0,
//       receive_buffer: None,
//       message: message.clone(),
//       stream_id,
//       finished: false,
//       tool_calls: Vec::new(),
//       wrapped_content: String::new(),
//       stylized: Rope::new(),
//       tools_called: false,
//       response_count: 0,
//       token_usage: 0,
//     }
//   }
// }
//
impl AsRef<ChatMessage> for ChatMessage {
  fn as_ref(&self) -> &ChatMessage {
    self
  }
}
