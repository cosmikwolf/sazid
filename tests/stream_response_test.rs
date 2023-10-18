#[macro_use]
extern crate lazy_static;

#[cfg(test)]
mod tests {
  use sazid::app::types::*;
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
    let parsed = serde_json::from_str::<ChatTransaction>(&contents).unwrap();
    match parsed.clone() {
      ChatTransaction::StreamResponse(txn) => {
        print!("{:?}", txn.choices.len());
      },
      _ => {
        panic!("expected ChatTransaction::StreamResponse, got {:?}", parsed);
      },
    }
    let mut content = String::new();
    let messages = <Vec<RenderedChatMessage>>::from(parsed);
    for message in messages {
      content += message.content.clone().as_str();
      print!("{}", content);
    }
  }
}
