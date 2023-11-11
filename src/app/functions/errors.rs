use std::{error::Error, fmt, io};

use crate::trace_dbg;

#[derive(Debug)]
pub struct FunctionCallError {
  message: String,
  source: Option<Box<dyn Error>>,
}

impl FunctionCallError {
  pub fn new(message: &str) -> Self {
    trace_dbg!("FunctionCallError: {}", message);
    FunctionCallError { message: message.to_string(), source: None }
  }
}

// Implement the Display trait for your custom error type.
impl fmt::Display for FunctionCallError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "FunctionCallError: {}", self.message)
  }
}

// Implement the Error trait for your custom error type.
impl Error for FunctionCallError {
  fn description(&self) -> &str {
    &self.message
  }

  fn source(&self) -> Option<&(dyn Error + 'static)> {
    self.source.as_ref().map(|e| e.as_ref())
  }
}

impl From<grep::regex::Error> for FunctionCallError {
  fn from(error: grep::regex::Error) -> Self {
    FunctionCallError { message: format!("Grep Regex Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<serde_json::Error> for FunctionCallError {
  fn from(error: serde_json::Error) -> Self {
    FunctionCallError { message: format!("Serde JSON Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<io::Error> for FunctionCallError {
  fn from(error: io::Error) -> Self {
    FunctionCallError { message: format!("IO Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<String> for FunctionCallError {
  fn from(message: String) -> Self {
    FunctionCallError { message, source: None }
  }
}
