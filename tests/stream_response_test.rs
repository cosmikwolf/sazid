extern crate lazy_static;

#[cfg(test)]
mod tests {
  use async_openai::types::CreateChatCompletionStreamResponse;
  use sazid::app::types::*;
  use sazid::components::session::*;

  use std::fs::File;
  use std::io::Read;
  use std::path::PathBuf;
  // a test that reads in the file tests/assets/saved_stream_response.json and parses it
  // asserting that it is a ChatTransaction::StreamResponse
  #[test]
  fn test_stream_response_parsing() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/assets/saved_stream_response.json");
    let mut file = File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let parsed = serde_json::from_str::<CreateChatCompletionStreamResponse>(&contents).unwrap();
    insta::assert_toml_snapshot!(&parsed);
    //  match parsed {
    //    ChatTransaction::StreamResponse(txn) => {},
    //    _ => {
    //      panic!("expected ChatTransaction::StreamResponse, got {:?}", parsed)
    //    },
    //  }
    // let mut content = String::new();
    // let messages = <Vec<RenderedChatMessage>>::from(parsed);
    // for message in messages {
    //   content += message.content.clone().as_str();
    //   print!("{}", content);
    // }
  } //

  #[test]
  fn test_construct_chat_completion_request_message() {
    let session = Session::new();
    if let Ok(create_chat_completion_request_message_result) =
      construct_chat_completion_request_message("testing testing 1 2 3", &session.config.model)
    {
      insta::assert_toml_snapshot!(create_chat_completion_request_message_result);
    } else {
      panic!("construct_chat_completion_request_message failed")
    };
  }
}
