use serde_derive::{Deserialize, Serialize};

use super::messages::{ChatMessage, MessageContainer, ReceiveBuffer};

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
        if let Some(MessageContainer { receive_buffer: Some(ReceiveBuffer::StreamResponse(srvec)), .. }) =
          self.messages.iter_mut().find(|m| m.stream_id == Some(new_srvec[0].id.clone()))
        {
          for sr in new_srvec {
            srvec.push(sr);
          }
        } else {
          self.messages.push(MessageContainer::new_from_receive_buffer(ReceiveBuffer::StreamResponse(new_srvec)));
        }
      },
      _ => {
        self.messages.push(message.clone().into());
      },
    };
    // return a vec of any functions that need to be called
  }
}
