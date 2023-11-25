use std::{
  collections::HashSet,
  fmt::{self, Formatter},
};

use color_eyre::owo_colors::OwoColorize;
use crossterm::style::Stylize;
use ropey::Rope;
use serde_derive::{Deserialize, Serialize};

use crate::app::helpers::concatenate_create_chat_completion_stream_response;

use async_openai::{
  self,
  types::{
    ChatCompletionMessageToolCallChunk, ChatCompletionRequestAssistantMessage, ChatCompletionRequestFunctionMessage,
    ChatCompletionRequestMessageContentPart, ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
    ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent, CreateChatCompletionResponse,
    CreateChatCompletionStreamResponse, FunctionCall, FunctionCallStream, Role,
  },
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MessageContainer {
  pub message: ChatMessage,
  pub stream_id: Option<String>,
  pub rendered: RenderedChatMessage,
  pub finished: bool,
  pub tool_calls: Vec<ChatCompletionMessageToolCall>,
  pub function_called: bool,
  pub response_count: usize,
  pub token_usage: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ChatMessage {
  Response(CreateChatCompletionResponse),
  StreamResponse(Vec<CreateChatCompletionStreamResponse>),
  SazidMessage(String),
  System(ChatCompletionRequestSystemMessage),
  User(ChatCompletionRequestUserMessage),
  Assistant(ChatCompletionRequestAssistantMessage),
  Tool(ChatCompletionRequestToolMessage),
  Function(ChatCompletionRequestFunctionMessage),
}

impl fmt::Display for ChatMessage {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{}",
      match self {
        ChatMessage::SazidMessage(message) => format!("{}\n{}", "Sazid:".cyan(), message),
        ChatMessage::System(message) => match message.content {
          Some(content) => {
            format!("{}\n{}", "System:".bright_magenta(), content)
          },
          None => {
            format!("{}\n{}", "System:".bright_magenta(), "no content")
          },
        },
        ChatMessage::User(message) => match message.content {
          Some(ChatCompletionRequestUserMessageContent::Text(content)) => {
            format!("{}\n{}", "You:".bright_blue(), content)
          },
          Some(ChatCompletionRequestUserMessageContent::Array(parts)) => {
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
          },
          None => {
            format!("{}\n{}", "You:".bright_blue(), "no content")
          },
        },
        ChatMessage::Assistant(message) => {
          let mut content: Vec<String> = Vec::new();
          content.push(match message.content {
            Some(content) => format!("{}\n{}\n", "Assistant:".bright_yellow(), content),
            None => format!("{}\n{}\n", "Assistant:".bright_yellow(), "no content"),
          });
          match message.tool_calls {
            Some(tool_calls) => {
              for tool_call in tool_calls {
                content.push(format!("{}\n{}", "Tool:".bright_green(), tool_call.function.name));
                content.push(format!("{}\n{}", "Arguments:".bright_green(), tool_call.function.arguments));
              }
            },
            None => {},
          }
          content.join("\n")
        },
        ChatMessage::Tool(message) => {
          let mut content: Vec<String> = Vec::new();
          content.push(format!("{}\n{}", "Tool:".bright_green(), message.tool_call_id));
          content.push(match message.content {
            Some(content) => format!("{}", content),
            None => format!("{}", "no content"),
          });
          content.join("\n")
        },
        ChatMessage::Function(message) => {
          let mut content: Vec<String> = Vec::new();
          content.push(format!("{}\n{}", "Function:".bright_green(), message.name));
          content.push(match message.content {
            Some(content) => format!("{}", content),
            None => format!("{}", "no content"),
          });
          content.join("\n")
        },
        ChatMessage::Response(message) => {
          let mut content: Vec<String> = Vec::new();
          for choice in &message.choices {
            if &message.choices.len() > &1 {
              content.push(format!("{}\n{}", "Choice #".bright_green(), choice.index));
            }
            content.push(match choice.message.content {
              Some(content) => format!("{}\n{}", "Assistant:".bright_yellow(), content),
              None => format!("{}\n{}", "Assistant:".bright_yellow(), "no content"),
            });
            match choice.message.tool_calls {
              Some(tool_calls) => {
                for tool_call in tool_calls {
                  content.push(format!("{}\n{}", "Tool:".bright_green(), tool_call.function.name));
                  content.push(format!("{}\n{}", "Arguments:".bright_green(), tool_call.function.arguments));
                }
              },
              None => {},
            };
            if &message.choices.len() > &1 {
              content.push("\n".to_string());
            }
          }
          content.join("\n")
        },
        ChatMessage::StreamResponse(messages) => {
          let mut content: Vec<String> = Vec::new();
          let message = messages
            .iter()
            .skip(1)
            .try_fold(messages[0], |acc, m| concatenate_create_chat_completion_stream_response(&acc, m))
            .unwrap();

          let mut choice_idxs = message.choices.iter().map(|c| c.index as usize).collect::<Vec<usize>>();
          choice_idxs.sort_unstable();
          choice_idxs.dedup();

          choice_idxs.iter().for_each(|choice_idx| {
            if choice_idxs.len() > 1 {
              content.push(format!("{}{}:", "Choice #".bright_green(), choice_idx));
            }
            let mut tool_call_chunks: Vec<ChatCompletionMessageToolCallChunk> = Vec::new();
            message.choices.iter().filter(|c| c.index as usize == *choice_idx).for_each(|choice| {
              content.push(match choice.delta.content {
                Some(content) => format!("{}\n{}", "Assistant:".bright_yellow(), content),
                None => format!("{}\n{}", "Assistant:".bright_yellow(), "no content"),
              });

              match choice.delta.tool_calls {
                Some(tool_calls) => {
                  for tool_call in tool_calls {
                    tool_call_chunks.push(tool_call.clone());
                  }
                },
                None => {},
              };
            });
            tool_call_chunks.iter().map(|tc| tc.index as usize).collect::<Vec<usize>>().iter().for_each(
              |tool_call_idx| {
                //tool_call_chunks.iter().filter(|tc| tc.index == tool_call_idx).skip(1).try_fold(tool_call_chunks[0], |acc, tc| concatenate_tool_call_chunks(&acc, tc) )
                let tool_call_chunks_by_idx = tool_call_chunks
                  .iter()
                  .filter(|tc| tc.index as usize == *tool_call_idx)
                  .collect::<Vec<&ChatCompletionMessageToolCallChunk>>();

                let id = tool_call_chunks_by_idx.iter().flat_map(|tc| tc.id).collect::<Vec<String>>().join(" ");

                let name = tool_call_chunks_by_idx
                  .iter()
                  .flat_map(|tc| tc.function)
                  .flat_map(|fc| fc.name)
                  .collect::<Vec<String>>()
                  .join(" ");

                let arguments = tool_call_chunks_by_idx
                  .iter()
                  .flat_map(|tc| tc.function)
                  .flat_map(|fc| fc.name)
                  .collect::<Vec<String>>()
                  .join(" ");

                content.push(format!("{}{}", "Tool ID:".bright_green(), id));
                content.push(format!("{}\t{}", "Name:".bright_green(), name));
                content.push(format!("{}\n{}", "Arguments:".bright_green(), arguments));
              },
            );
          });
          content.join("\n")
        },
      }
    );
    Ok(())
  }
}

impl MessageContainer {
  pub fn is_finished(&self) -> bool {
    match self.message {
      ChatMessage::Response(response) => response.choices.iter().all(|c| c.finish_reason.is_some()),
      ChatMessage::StreamResponse(srvec) => {
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
    }
  }
  pub fn render_message(&mut self, window_width: usize) {
    self.rendered = RenderedChatMessage::from(&self.message);
  }
}
impl MessageContainer {
  // pub fn get_token_count(&self) -> usize {
  //   if self.token_usage == 0 {
  //     self.set_token_count();
  //   }
  //   self.token_usage
  // }
  //
  // pub fn set_token_count(&mut self) {
  //   let bpe = tiktoken_rs::cl100k_base().unwrap();
  //   self.token_usage = bpe.encode_with_special_tokens(self.rendered.stylized.to_string().as_str()).len()
  // }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct RenderedChatMessage {
  pub role: Option<Role>,
  pub content: String,
  pub wrapped_content: String,
  #[serde(skip)]
  pub stylized: Rope,
  //pub function_call: Option<RenderedFunctionCall>,
  pub name: Option<String>,
  pub finished: bool,
  pub token_usage: usize,
}

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
// pub enum ChatResponse {
//   Response(CreateChatCompletionResponse),
//   StreamResponse(CreateChatCompletionStreamResponse),
// }
//
// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
// pub enum ChatResponseSingleMessage {
//   Response(ChatChoice),
//   StreamResponse(Vec<ChatCompletionResponseStreamMessage>),
// }
//
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
    let stream_id = match message {
      ChatMessage::StreamResponse(srvec) => Some(srvec[0].id.clone()),
      _ => None,
    };
    MessageContainer {
      message: message.clone(),
      stream_id,
      rendered: RenderedChatMessage::from(&message),
      finished: false,
      tool_calls: Vec::new(),
      function_called: false,
      response_count: 0,
      token_usage: 0,
    }
  }
}

// impl From<ChatResponse> for Vec<ChatMessage> {
//   fn from(response: ChatResponse) -> Self {
//     let mut messages: Vec<ChatMessage> = Vec::new();
//     match response {
//       ChatResponse::Response(response) => response.choices.iter().for_each(|choice| {
//         messages.push(ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::Response(choice.clone())))
//       }),
//       ChatResponse::StreamResponse(response) => messages
//         .push(ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(response.choices))),
//     }
//     messages
//   }
// }

// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
// pub enum ChatMessage {
//   SazidSystemMessage(String),
//   FunctionResult(FunctionResult),
//   ToolResult(ChatCompletionRequestToolMessage),
//   PromptMessage(ChatCompletionRequestMessage),
//   UserPromptMessage(ChatCompletionRequestMessage),
//   ChatCompletionRequestMessage(ChatCompletionRequestMessage),
//   ChatCompletionResponseMessage(ChatResponseSingleMessage),
// }

// impl From<&ChatMessage> for Option<MessageTypes> {
//   fn from(message: &ChatMessage) -> Self {
//     match message {
//       ChatMessage::SazidSystemMessage(_) => None,
//       ChatMessage::FunctionResult(result) => {
//         Some(ChatCompletionMessage::Request(ChatCompletionRequestMessage::Tool(ChatCompletionRequestToolMessage {
//           tool_call_id: result.name.clone(),
//           role: Role::Function,
//           content: Some(result.response.clone()),
//         })))
//       },
//       ChatMessage::PromptMessage(request) => Some(ChatCompletionMessage::Request(request.clone())),
//       ChatMessage::ChatCompletionRequestMessage(request) => Some(ChatCompletionMessage::Request(request.clone())),
//       ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::StreamResponse(srvec)) => Some({
//         let mut message = srvec[0].clone();
//         srvec.iter().skip(1).for_each(|sr| {
//           message = concatenate_stream_response_messages(&message, sr);
//         });
//         ChatCompletionMessage::StreamResponse(message)
//       }),
//       ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::Response(response)) => Some({
//         ChatCompletionMessage::Response(ChatCompletionResponseMessage {
//           role: Role::Assistant,
//           content: response.message.content.clone(),
//           function_call: None,
//           tool_calls: None,
//         })
//       }),
//     }
//   }
// }
// //
impl AsRef<ChatMessage> for ChatMessage {
  fn as_ref(&self) -> &ChatMessage {
    self
  }
}

impl From<&ChatMessage> for RenderedChatMessage {
  fn from(message: &ChatMessage) -> Self {
    let content = message.to_string();
    match message {
      ChatMessage::SazidMessage(_) => RenderedChatMessage {
        name: None,
        role: None,
        content,
        wrapped_content: String::new(),
        stylized: Rope::new(),
        finished: true,
        token_usage: 0,
      },
      ChatMessage::Function(result) => RenderedChatMessage {
        name: Some(result.name.clone()),
        role: Some(Role::Function),
        content,
        wrapped_content: String::new(),
        stylized: Rope::new(),
        finished: true,
        token_usage: 0,
      },
      ChatMessage::Tool(request) => RenderedChatMessage {
        name: None,
        role: Some(Role::User),
        content,
        wrapped_content: String::new(),
        stylized: Rope::new(),
        finished: true,
        token_usage: 0,
      },
      ChatMessage::System(request) => RenderedChatMessage {
        name: None,
        role: Some(Role::User),
        content,
        wrapped_content: String::new(),
        stylized: Rope::new(),
        finished: true,
        token_usage: 0,
      },
      ChatMessage::User(request) => RenderedChatMessage {
        name: None,
        role: Some(Role::User),
        content,
        wrapped_content: String::new(),
        stylized: Rope::new(),
        finished: true,
        token_usage: 0,
      },
      ChatMessage::Assistant(request) => RenderedChatMessage {
        name: None,
        role: Some(Role::User),
        content,
        wrapped_content: String::new(),
        stylized: Rope::new(),
        finished: true,
        token_usage: 0,
      },
      ChatMessage::Response(response) => RenderedChatMessage {
        name: None,
        role: Some(Role::Assistant),
        content,
        wrapped_content: String::new(),
        stylized: Rope::new(),
        finished: response.choices.iter().all(|c| c.finish_reason.is_some()),
        token_usage: 0,
      },
      ChatMessage::StreamResponse(srvec) => {
        let mut message = srvec
          .iter()
          .skip(1)
          .try_fold(srvec[0], |acc, sr| concatenate_create_chat_completion_stream_response(&acc, sr))
          .unwrap();

        let mut choices_idxs = message.choices.iter().map(|c| c.index as usize).collect::<Vec<usize>>();
        choices_idxs.sort_unstable();
        choices_idxs.dedup();
        RenderedChatMessage {
          name: None,
          role: Some(Role::Assistant),
          content,
          wrapped_content: String::new(),
          stylized: Rope::new(),
          finished: choices_idxs.iter().all(|choice_idx| {
            message.choices.iter().any(|c| c.index as usize == *choice_idx && c.finish_reason.is_some())
          }),
          token_usage: 0,
        }
      },
    }
  }
}
