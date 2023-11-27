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
        new_srvec.iter().for_each(|sr| {
          if let Some(message) = self.messages.iter_mut().find(|m| {
            m.stream_id == Some(sr.id.clone()) && matches!(m.receive_buffer, Some(ReceiveBuffer::StreamResponse(_)))
          }) {
            message.update_stream_response(sr.clone()).unwrap();
          } else {
            self
              .messages
              .push(MessageContainer::new_from_receive_buffer(ReceiveBuffer::StreamResponse(vec![sr.clone()])));
          }
        });
      },
      _ => {
        self.messages.push(message.into());
      },
    };
    // return a vec of any functions that need to be called
  }
}
