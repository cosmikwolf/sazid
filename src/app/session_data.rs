use serde_derive::{Deserialize, Serialize};

use super::{
  helpers::collate_stream_response_vec,
  messages::{ChatMessage, ChatResponseSingleMessage, MessageContainer, RenderedChatMessage, RenderedFunctionCall},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SessionData {
  pub messages: Vec<MessageContainer>,
  pub window_width: usize,
}

impl Default for SessionData {
  fn default() -> Self {
    SessionData { messages: vec![], window_width: 80 }
  }
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
            //trace_dbg!("add_message: existing delta \n{:?}\n{:?}", new_srvec, existing_srvec);
            collate_stream_response_vec(new_srvec, existing_srvec);
          } else {
            //trace_dbg!("add_message: new delta {:?}", new_srvec);
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
}
