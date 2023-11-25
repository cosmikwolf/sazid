use serde_derive::{Deserialize, Serialize};

use super::{
  helpers::collate_stream_response_vec,
  messages::{ChatMessage, MessageContainer, RenderedChatMessage, RenderedFunctionCall},
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
      ChatMessage::User(_) => self.messages.push(message.into()),
      ChatMessage::StreamResponse(new_srvec) => {
        if let Some(MessageContainer { message: ChatMessage::StreamResponse(srvec), .. }) =
          self.messages.iter().find(|m| m.stream_id == Some(new_srvec[0].id))
        {
          for sr in new_srvec {
            srvec.push(sr);
          }
        } else {
          self.messages.push(message.into());
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
      })
      .collect()
  }
}
