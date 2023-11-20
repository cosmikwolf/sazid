use std::{error::Error, fmt, io};

use crate::trace_dbg;

#[derive(Debug)]
pub struct ModelFunctionError {
  message: String,
  source: Option<Box<dyn Error>>,
}

impl ModelFunctionError {
  pub fn new(message: &str) -> Self {
    trace_dbg!("ModelFunctionError: {}", message);
    ModelFunctionError { message: message.to_string(), source: None }
  }
}

// Implement the Display trait for your custom error type.
impl fmt::Display for ModelFunctionError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

// Implement the Error trait for your custom error type.
impl Error for ModelFunctionError {
  fn description(&self) -> &str {
    &self.message
  }

  fn source(&self) -> Option<&(dyn Error + 'static)> {
    self.source.as_ref().map(|e| e.as_ref())
  }
}

impl From<grep::regex::Error> for ModelFunctionError {
  fn from(error: grep::regex::Error) -> Self {
    ModelFunctionError { message: format!("Grep Regex Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<serde_json::Error> for ModelFunctionError {
  fn from(error: serde_json::Error) -> Self {
    ModelFunctionError { message: format!("Serde JSON Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<io::Error> for ModelFunctionError {
  fn from(error: io::Error) -> Self {
    ModelFunctionError { message: format!("IO Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<String> for ModelFunctionError {
  fn from(message: String) -> Self {
    ModelFunctionError { message, source: None }
  }
}
