use std::{
  collections::HashSet,
  fmt::{self, Formatter},
};

use color_eyre::owo_colors::OwoColorize;
use ropey::Rope;
use serde_derive::{Deserialize, Serialize};

use async_openai::{
  self,
  types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage, ChatCompletionRequestFunctionMessage,
    ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPart, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestToolMessage, ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse, FunctionCall, FunctionCallStream, Role,
  },
};

use super::{
  errors::ParseError,
  helpers::{
    get_assistant_message_from_create_chat_completion_response,
    get_assistant_message_from_create_chat_completion_stream_response,
  },
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MessageContainer {
  pub message: ChatCompletionRequestMessage,
  pub receive_buffer: Option<ReceiveBuffer>,
  pub tool_calls: Vec<ChatCompletionMessageToolCall>,
  pub stream_id: Option<String>,
  pub selected_choice: usize,
  pub tools_called: bool,
  pub receive_complete: bool,
  pub stylize_complete: bool,
  pub response_count: usize,
  pub wrapped_content: String,
  #[serde(skip)]
  pub stylized: Rope,
  pub token_usage: usize,
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

impl From<ChatMessage> for MessageContainer {
  fn from(message: ChatMessage) -> Self {
    match message {
      ChatMessage::Response(response) => MessageContainer::new_from_receive_buffer(ReceiveBuffer::Response(response)),
      ChatMessage::StreamResponse(response) => {
        MessageContainer::new_from_receive_buffer(ReceiveBuffer::StreamResponse(response))
      },
      ChatMessage::Tool(message) => MessageContainer::new(ChatCompletionRequestMessage::Tool(message)),
      ChatMessage::Function(message) => MessageContainer::new(ChatCompletionRequestMessage::Function(message)),
      ChatMessage::System(message) => MessageContainer::new(ChatCompletionRequestMessage::System(message)),
      ChatMessage::User(message) => MessageContainer::new(ChatCompletionRequestMessage::User(message)),
      ChatMessage::Assistant(message) => MessageContainer::new(ChatCompletionRequestMessage::Assistant(message)),
    }
  }
}
impl fmt::Display for MessageContainer {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{}",
      match &self.message {
                ChatCompletionRequestMessage::System(message) => match &message
                    .content
                {
                    Some(content) => {
                        format!("{}\n{}", "System:".bright_magenta(), content)
                    }
                    None => {
                        format!(
                            "{}\n{}",
                            "System:".bright_magenta(),
                            "no content"
                        )
                    }
                },
                ChatCompletionRequestMessage::User(message) => match &message
                    .content
                {
                    Some(ChatCompletionRequestUserMessageContent::Text(
                        content,
                    )) => {
                        format!("{}\n{}", "You:".bright_blue(), content)
                    }
                    Some(ChatCompletionRequestUserMessageContent::Array(
                        parts,
                    )) => {
                        let mut content: Vec<String> = Vec::new();
                        for part in parts {
                            content.push(match part {
                ChatCompletionRequestMessageContentPart::Text(content) => {
                  format!("{}\n{}", "You:".bright_blue(), content.text)
                },
                ChatCompletionRequestMessageContentPart::Image(content) => {
                  format!("{}\n{}", "You <Image>:".bright_blue(), content.image_url.url)
                },
              })
                        }
                        content.join("\n")
                    }
                    None => {
                        format!("{}\n{}", "You:".bright_blue(), "no content")
                    }
                },
                ChatCompletionRequestMessage::Assistant(message) => {
                    let mut content: Vec<String> = Vec::new();
                    content.push(match &message.content {
                        Some(content) => format!(
                            "{}\n{}\n",
                            "Assistant:".bright_yellow(),
                            content
                        ),
                        None => format!(
                            "{}\n{}\n",
                            "Assistant:".bright_yellow(),
                            "no content"
                        ),
                    });
                    match &message.tool_calls {
                        Some(tool_calls) => {
                            for tool_call in tool_calls {
                                content.push(format!(
                                    "{}\n{}",
                                    "Tool:".bright_green(),
                                    tool_call.function.name
                                ));
                                content.push(format!(
                                    "{}\n{}",
                                    "Arguments:".bright_green(),
                                    tool_call.function.arguments
                                ));
                            }
                        }
                        None => {}
                    }
                    content.join("\n")
                }
                ChatCompletionRequestMessage::Tool(message) => {
                    let mut content: Vec<String> = Vec::new();
                    content.push(format!(
                        "{}\n{}",
                        "Tool:".bright_green(),
                        message.tool_call_id
                    ));
                    content.push(match &message.content {
                        Some(content) => format!("{}", content),
                        None => format!("{}", "no content"),
                    });
                    content.join("\n")
                }
                ChatCompletionRequestMessage::Function(message) => {
                    let mut content: Vec<String> = Vec::new();
                    content.push(format!(
                        "{}\n{}",
                        "Function:".bright_green(),
                        message.name
                    ));
                    content.push(match &message.content {
                        Some(content) => format!("{}", content),
                        None => format!("{}", "no content"),
                    });
                    content.join("\n")
                }
                // ChatMessage::Response(message) => {
                //   let mut content: Vec<String> = Vec::new();
                //   let choice = &message.choices[self.selected_choice];
                //   if &message.choices.len() > &1 {
                //     content.push(format!("{}\n{}", "Choice #".bright_green(), choice.index));
                //   }
                //   content.push(match choice.message.content {
                //     Some(content) => format!("{}\n{}", "Assistant:".bright_yellow(), content),
                //     None => format!("{}\n{}", "Assistant:".bright_yellow(), "no content"),
                //   });
                //   match choice.message.tool_calls {
                //     Some(tool_calls) => {
                //       for tool_call in tool_calls {
                //         content.push(format!("{}\n{}", "Tool:".bright_green(), tool_call.function.name));
                //         content.push(format!("{}\n{}", "Arguments:".bright_green(), tool_call.function.arguments));
                //       }
                //     },
                //     None => {},
                //   };
                //   if &message.choices.len() > &1 {
                //     content.push("\n".to_string());
                //   }
                //   content.join("\n")
                // },
                // ChatMessage::StreamResponse(messages) => {
                //   let mut content: Vec<String> = Vec::new();
                //   let message = messages
                //     .iter()
                //     .skip(1)
                //     .try_fold(messages[0], |acc, m| concatenate_create_chat_completion_stream_response(&acc, m))
                //     .unwrap();
                //
                //   let mut choice_idxs = message.choices.iter().map(|c| c.index as usize).collect::<Vec<usize>>();
                //   choice_idxs.sort_unstable();
                //   choice_idxs.dedup();
                //
                //   if choice_idxs.len() > 1 {
                //     content.push(format!("{}{}:", "Choice #".bright_green(), self.selected_choice));
                //   }
                //   let mut tool_call_chunks: Vec<ChatCompletionMessageToolCallChunk> = Vec::new();
                //   message.choices.iter().filter(|c| c.index as usize == self.selected_choice).for_each(|choice| {
                //     content.push(match choice.delta.content {
                //       Some(content) => format!("{}\n{}", "Assistant:".bright_yellow(), content),
                //       None => format!("{}\n{}", "Assistant:".bright_yellow(), "no content"),
                //     });
                //
                //     match choice.delta.tool_calls {
                //       Some(tool_calls) => {
                //         for tool_call in tool_calls {
                //           tool_call_chunks.push(tool_call.clone());
                //         }
                //       },
                //       None => {},
                //     };
                //   });
                //   tool_call_chunks.iter().map(|tc| tc.index as usize).collect::<Vec<usize>>().iter().for_each(
                //     |tool_call_idx| {
                //       //tool_call_chunks.iter().filter(|tc| tc.index == tool_call_idx).skip(1).try_fold(tool_call_chunks[0], |acc, tc| concatenate_tool_call_chunks(&acc, tc) )
                //       let tool_call_chunks_by_idx = tool_call_chunks
                //         .iter()
                //         .filter(|tc| tc.index as usize == *tool_call_idx)
                //         .collect::<Vec<&ChatCompletionMessageToolCallChunk>>();
                //
                //       let id = tool_call_chunks_by_idx.iter().flat_map(|tc| tc.id).collect::<Vec<String>>().join(" ");
                //
                //       let name = tool_call_chunks_by_idx
                //         .iter()
                //         .flat_map(|tc| tc.function)
                //         .flat_map(|fc| fc.name)
                //         .collect::<Vec<String>>()
                //         .join(" ");
                //
                //       let arguments = tool_call_chunks_by_idx
                //         .iter()
                //         .flat_map(|tc| tc.function)
                //         .flat_map(|fc| fc.name)
                //         .collect::<Vec<String>>()
                //         .join(" ");
                //
                //       content.push(format!("{}{}", "Tool ID:".bright_green(), id));
                //       content.push(format!("{}\t{}", "Name:".bright_green(), name));
                //       content.push(format!("{}\n{}", "Arguments:".bright_green(), arguments));
                //     },
                //   );
                //   content.join("\n")
                // },
            }
    );
    Ok(())
  }
}

impl MessageContainer {
  pub fn new(message: ChatCompletionRequestMessage) -> Self {
    MessageContainer {
      message,
      receive_buffer: None,
      tool_calls: Vec::new(),
      stream_id: None,
      selected_choice: 0,
      stylize_complete: false,
      receive_complete: false,
      wrapped_content: String::new(),
      stylized: Rope::new(),
      tools_called: false,
      response_count: 0,
      token_usage: 0,
    }
  }

  pub fn new_from_receive_buffer(receive_buffer: ReceiveBuffer) -> Self {
    match &receive_buffer {
      ReceiveBuffer::Response(response) => MessageContainer {
        message: ChatCompletionRequestMessage::Assistant(
          get_assistant_message_from_create_chat_completion_response(0, response).unwrap(),
        ),
        receive_buffer: Some(receive_buffer.clone()),
        tool_calls: Vec::new(),
        stream_id: None,
        selected_choice: 0,
        stylize_complete: false,
        receive_complete: false,
        wrapped_content: String::new(),
        stylized: Rope::new(),
        tools_called: false,
        response_count: 0,
        token_usage: 0,
      },
      ReceiveBuffer::StreamResponse(response) => MessageContainer {
        message: ChatCompletionRequestMessage::Assistant(
          get_assistant_message_from_create_chat_completion_stream_response(0, &response).unwrap(),
        ),
        receive_buffer: Some(receive_buffer.clone()),
        tool_calls: Vec::new(),
        stream_id: Some(response[0].id.clone()),
        selected_choice: 0,
        stylize_complete: false,
        receive_complete: false,
        wrapped_content: String::new(),
        stylized: Rope::new(),
        tools_called: false,
        response_count: 0,
        token_usage: 0,
      },
    }
  }

  pub fn update_stream_response(
    &mut self,
    stream_message: CreateChatCompletionStreamResponse,
  ) -> Result<(), ParseError> {
    if self.stream_id == Some(stream_message.id.clone()) {
      match &mut self.receive_buffer {
        Some(ReceiveBuffer::StreamResponse(srvec)) => {
          srvec.push(stream_message);

          self.message = ChatCompletionRequestMessage::Assistant(
            get_assistant_message_from_create_chat_completion_stream_response(self.selected_choice, &srvec).unwrap(),
          );
          self.parse_response_buffer();
          Ok(())
        },
        _ => Err(ParseError::new("MessageContainer::update_stream_response: message is not a stream response")),
      }
    } else {
      Err(ParseError::new("MessageContainer::update_stream_response: stream id does not match"))
    }
  }

  pub fn parse_response_buffer(&mut self) {
    if match &self.receive_buffer {
      Some(ReceiveBuffer::Response(response)) => response.choices.iter().all(|c| c.finish_reason.is_some()),
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
        srvec
          .iter()
          .all(|response| response.choices.iter().all(|choice| indexes_with_finish_reason.contains(&choice.index)))
      },
      _ => true,
    } {
      self.receive_complete = true;
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
    RenderedFunctionCall { name: function_call.name, arguments: function_call.arguments }
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
