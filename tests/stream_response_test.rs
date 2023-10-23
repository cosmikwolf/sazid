extern crate lazy_static;

#[cfg(test)]
mod tests {
  use async_openai::types::Role;
  use ntest::timeout;
  use sazid::action::Action;
  use sazid::app::types::{ChatResponse, Transaction};
  use sazid::components::session::*;
  use tokio::sync::mpsc;

  #[tokio::test]
  #[timeout(10000)]
  pub async fn test_request_response() {
    let mut enter_processing_action_run = false;
    let mut process_response_action_run = false;
    let (tx, mut rx) = mpsc::unbounded_channel::<Action>();
    let mut session = Session::new();
    session.request_response("Hello World".to_string(), tx.clone());
    'outer: loop {
      while let Some(res) = rx.recv().await {
        match res {
          Action::EnterProcessing => {
            enter_processing_action_run = true;
          },
          Action::ProcessResponse(boxed_id_response) => {
            let (transaction_id, response) = *boxed_id_response;
            process_response_action_run = true;
            session.process_response_handler(tx.clone(), transaction_id, response.clone());
            if let ChatResponse::StreamResponse(message) = response {
              insta::assert_yaml_snapshot!(&message, { ".id" => "[id]", ".created"  => "[created]" });
            } else {
              panic!("Expected StreamResponse");
            };
          },
          Action::ExitProcessing => {
            // break;
            if let Some(ChatResponse::StreamResponse(combined)) = session.transactions.last().unwrap().responses.last()
            {
              assert!(process_response_action_run);
              assert!(enter_processing_action_run);
              insta::assert_yaml_snapshot!(&combined, { ".id" => "[id]", ".created"  => "[created]" });
              insta::assert_yaml_snapshot!(&session.transactions.last().unwrap(), { ".id" => "[id]", ".created"  => "[created]" });
            } else {
              panic!(
                "Expected last transaction message to be StreamResponse {:#?}",
                session.transactions.last_mut().unwrap()
              );
            }
            break 'outer;
          },
          Action::Update => {},
          Action::Render => {},
          _ => {
            panic!("Unexpected action {:#?}", res);
          },
        }
      }
    }
  }

  #[test]
  fn test_construct_chat_completion_request_message() {
    let session = Session::new();
    if let Ok(create_chat_completion_request_message_result) =
      construct_chat_completion_request_message("testing testing 1 2 3", Role::User, &session.config.model, None, None)
    {
      insta::assert_toml_snapshot!(create_chat_completion_request_message_result);
    } else {
      panic!("construct_chat_completion_request_message failed")
    };
  }
}
