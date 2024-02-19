extern crate lazy_static;

#[cfg(test)]
mod tests {
  use async_openai::types::Role;
  use ntest::timeout;
  use sazid::action::Action;
  use sazid::app::messages::{
    ChatMessage, ChatResponseSingleMessage, MessageContainer,
  };
  use sazid::components::session::*;
  use tokio::sync::mpsc;

  #[tokio::test]
  #[timeout(10000)]
  pub async fn test_submit_chat_completion_request() {
    let mut enter_processing_action_run = false;
    let (tx, mut rx) = mpsc::unbounded_channel::<Action>();
    let mut session = Session::new();
    session
      .submit_chat_completion_request("Hello World".to_string(), tx.clone());
    'outer: loop {
      while let Some(res) = rx.recv().await {
        match res {
          Action::EnterProcessing => {
            enter_processing_action_run = true;
          },
          Action::ExitProcessing => {
            if let Some(MessageContainer {
              message:
                ChatMessage::ChatCompletionResponseMessage(
                  ChatResponseSingleMessage::StreamResponse(combined),
                ),
              ..
            }) = session.data.messages.last()
            {
              assert!(enter_processing_action_run);
              insta::assert_yaml_snapshot!(&combined, { ".id" => "[id]", ".created"  => "[created]" });
              insta::assert_yaml_snapshot!(&session.data.messages.last().unwrap(), { ".id" => "[id]", ".created"  => "[created]" });
              insta::assert_yaml_snapshot!(&session.data.messages, { ".id" => "[id]", ".created"  => "[created]" });
              insta::assert_yaml_snapshot!(&session.data);
              insta::assert_yaml_snapshot!(&session
                .view
                .rendered_text
                .to_string());
            } else {
              panic!(
                "Expected last transaction message to be StreamResponse {:#?}",
                session.data.messages.last_mut().unwrap()
              );
            }
            break 'outer;
          },
          Action::AddMessage(chat_message) => {
            insta::assert_yaml_snapshot!(&chat_message, { ".id" => "[id]", ".created"  => "[created]" });
            session.data.add_message(chat_message);
          },
          Action::RequestChatCompletion() => {
            session.request_chat_completion(tx.clone());
          },
          Action::Update => {},
          Action::Render => {},
          _ => {
            print!("Unexpected action {:#?}", res);
            panic!("Unexpected action {:#?}", res);
          },
        }
      }
    }
  }

  #[test]
  fn test_construct_chat_completion_request_message() {
    let mut session = Session::new();
    match session.add_chunked_chat_completion_request_messages(
      "testing testing 1 2 3",
      "sazid testing",
      Role::User,
      session.config.model.clone().as_ref(),
      None,
    ) {
      Ok(_) => {
        print!("{:?}", session.data);
        insta::assert_yaml_snapshot!(&session.data);
      },
      Err(e) => {
        print!("Error: {:#?}", e);
        panic!("Error: {:#?}", e);
      },
    };
  }
}
