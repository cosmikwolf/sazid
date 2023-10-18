extern crate sazid;
pub mod app;

#[cfg(test)]
mod tests {
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
    let transaction: ChatTransaction;
    file.read_to_string(&mut contents).unwrap();
    let parsed = serde_json::from_str::<ChatTransaction>(&contents).unwrap();
    match parsed {
      crate::ChatTransaction::StreamResponse { .. } => {},
      _ => panic!("Parsed transaction was not a StreamResponse"),
    }
  }
}
