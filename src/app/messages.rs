use ropey::Rope;
use serde_derive::{Deserialize, Serialize};

use super::helpers::concatenate_stream_response_messages;
use async_openai::{
  self,
  types::{
    ChatChoice, ChatCompletionRequestMessage, ChatCompletionResponseStreamMessage, CreateChatCompletionResponse,
    CreateChatCompletionStreamResponse, FinishReason, FunctionCall, FunctionCallStream, Role,
  },
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MessageContainer {
  pub message: ChatMessage,
  pub rendered: RenderedChatMessage,
  pub finished: bool,
  pub function_called: bool,
  pub response_count: usize,
}

impl MessageContainer {
  pub fn get_token_count(&self) -> usize {
    let bpe = tiktoken_rs::cl100k_base().unwrap();
    bpe.encode_with_special_tokens(self.rendered.stylized.to_string().as_str()).len()
  }

  // pub fn render_message_pulldown_cmark(&mut self, format_responses_only: bool) {
  //   // self.rendered.stylized = Some(String::from(&self.rendered))
  //   if format_responses_only {
  //     if matches!(self.message, ChatMessage::ChatCompletionResponseMessage(_)) {
  //       self.rendered.stylized = Some(render_markdown_to_string(String::from(&self.rendered)))
  //     } else {
  //       self.rendered.stylized = Some(String::from(&self.rendered))
  //     }
  //   } else {
  //     self.rendered.stylized = Some(render_markdown_to_string(String::from(&self.rendered)))
  //   }
  // }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct RenderedChatMessage {
  pub role: Option<Role>,
  pub content: String,
  pub wrapped_content: String,
  #[serde(skip)]
  pub stylized: Rope,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ChatMessage {
  SazidSystemMessage(String),
  FunctionResult(FunctionResult),
  PromptMessage(ChatCompletionRequestMessage),
  ChatCompletionRequestMessage(ChatCompletionRequestMessage),
  ChatCompletionResponseMessage(ChatResponseSingleMessage),
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
    MessageContainer {
      message: message.clone(),
      rendered: RenderedChatMessage::from(&message),
      finished: false,
      function_called: false,
      response_count: 0,
    }
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
          // todo: this MIGHT be a problem...
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
        content: content.clone(),
        wrapped_content: String::new(),
        stylized: Rope::new(),
        function_call: None,
        finish_reason: Some(FinishReason::Stop),
      },
      ChatMessage::FunctionResult(result) => RenderedChatMessage {
        name: Some(result.name.clone()),
        role: Some(Role::Function),
        content: result.response.clone(),
        wrapped_content: String::new(),
        stylized: Rope::new(),
        function_call: None,
        finish_reason: Some(FinishReason::Stop),
      },
      ChatMessage::PromptMessage(request) => RenderedChatMessage {
        name: None,
        role: Some(request.role),
        content: format!("## Prompt\n\n{}", request.content.clone().unwrap_or("no prompt".to_string())),
        wrapped_content: String::new(),
        stylized: Rope::new(),
        function_call: None,
        finish_reason: Some(FinishReason::Stop),
      },
      ChatMessage::ChatCompletionRequestMessage(request) => RenderedChatMessage {
        name: None,
        role: Some(request.role),
        content: request.content.clone().unwrap_or_default(),
        wrapped_content: String::new(),
        stylized: Rope::new(),
        function_call: request.function_call.clone().map(|function_call| function_call.into()),
        finish_reason: Some(FinishReason::Stop),
      },
      ChatMessage::ChatCompletionResponseMessage(ChatResponseSingleMessage::Response(response)) => {
        RenderedChatMessage {
          name: None,
          role: Some(Role::Assistant),
          content: response.message.content.clone().unwrap_or_default(),
          wrapped_content: String::new(),
          stylized: Rope::new(),
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
          content: message.delta.content.unwrap_or_default(),
          wrapped_content: String::new(),
          stylized: Rope::new(),
          function_call: message.delta.function_call.map(|function_call| function_call.into()),
          finish_reason: message.finish_reason,
        }
      },
    }
  }
}
