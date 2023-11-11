use serde_derive::{Deserialize, Serialize};

use super::{
  functions::function_call::RenderedFunctionCall,
  helpers::collate_stream_response_vec,
  messages::{ChatMessage, ChatResponseSingleMessage, MessageContainer, RenderedChatMessage},
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SessionData {
  pub messages: Vec<MessageContainer>,
  pub rendered_text: String,
  pub window_width: usize,
}

impl Default for SessionData {
  fn default() -> Self {
    SessionData { messages: vec![], rendered_text: String::new(), window_width: 80 }
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
    self.post_process_new_messages()
  }
  pub fn set_window_width(&mut self, width: usize) {
    if self.window_width != width {
      self.window_width = width;
      self.messages.iter_mut().for_each(|m| m.wrap_stylized_text(width));
    }
  }

  pub fn post_process_new_messages(&mut self) {
    self.rendered_text = self
      .messages
      .iter_mut()
      .flat_map(|message| {
        if !message.finished {
          // trace_dbg!("post_process_new_messages: processing message {:#?}", message.message);
          message.rendered = RenderedChatMessage::from(&ChatMessage::from(message.clone()));
          // message.render_message_pulldown_cmark(true);
          message.render_message_bat();
          message.wrap_stylized_text(self.window_width);
          if message.rendered.finish_reason.is_some() {
            message.finished = true;
            // trace_dbg!("post_process_new_messages: finished message {:#?}", message);
          }
        }
        message.rendered.wrapped_lines.iter().map(|wl| wl.as_str()).collect::<Vec<&str>>()
      })
      .collect::<Vec<&str>>()
      .join("\n");
  }

  pub fn get_display_text(&self, index: usize, count: usize) -> Vec<String> {
    self
      .messages
      .iter()
      .map(|m| m.rendered.wrapped_lines.clone())
      .skip(index)
      .take(count)
      .flatten()
      .collect::<Vec<String>>()
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
