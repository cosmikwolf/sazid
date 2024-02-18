use std::{error::Error, fmt, io};

use crate::trace_dbg;

#[derive(Debug)]
pub struct ToolCallError {
  message: String,
  source: Option<Box<dyn Error>>,
}

impl ToolCallError {
  pub fn new(message: &str) -> Self {
    trace_dbg!("ModelFunctionError: {}", message);
    ToolCallError { message: message.to_string(), source: None }
  }
}

// Implement the Display trait for your custom error type.
impl fmt::Display for ToolCallError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

// Implement the Error trait for your custom error type.
impl Error for ToolCallError {
  fn description(&self) -> &str {
    &self.message
  }

  fn source(&self) -> Option<&(dyn Error + 'static)> {
    self.source.as_ref().map(|e| e.as_ref())
  }
}
impl From<globset::Error> for ToolCallError {
  fn from(error: globset::Error) -> Self {
    ToolCallError {
      message: format!("Globset Error: {}", error),
      source: Some(Box::new(error)),
    }
  }
}
impl From<grep::regex::Error> for ToolCallError {
  fn from(error: grep::regex::Error) -> Self {
    ToolCallError {
      message: format!("Grep Regex Error: {}", error),
      source: Some(Box::new(error)),
    }
  }
}

impl From<serde_json::Error> for ToolCallError {
  fn from(error: serde_json::Error) -> Self {
    ToolCallError {
      message: format!("Serde JSON Error: {}", error),
      source: Some(Box::new(error)),
    }
  }
}

impl From<io::Error> for ToolCallError {
  fn from(error: io::Error) -> Self {
    ToolCallError {
      message: format!("IO Error: {}", error),
      source: Some(Box::new(error)),
    }
  }
}

impl From<String> for ToolCallError {
  fn from(message: String) -> Self {
    ToolCallError { message, source: None }
  }
}
