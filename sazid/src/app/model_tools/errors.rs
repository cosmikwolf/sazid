use std::{error::Error, fmt, io};

#[derive(Debug)]
pub struct ToolCallError {
  message: String,
  source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl ToolCallError {
  pub fn new(message: &str) -> Self {
    log::debug!("ModelFunctionError: {}", message);
    ToolCallError { message: message.to_string(), source: None }
  }

  pub fn source(&self) -> Option<&(dyn Error + Send + Sync + 'static)> {
    self.source.as_deref()
  }
}

// Implement the Display trait for your custom error type.
impl fmt::Display for ToolCallError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.message)
  }
}

// Implement the Error trait for your custom error type.
impl Error for ToolCallError {}

impl From<globset::Error> for ToolCallError {
  fn from(error: globset::Error) -> Self {
    ToolCallError { message: format!("Globset Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<grep::regex::Error> for ToolCallError {
  fn from(error: grep::regex::Error) -> Self {
    ToolCallError { message: format!("Grep Regex Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<serde_json::Error> for ToolCallError {
  fn from(error: serde_json::Error) -> Self {
    ToolCallError { message: format!("Serde JSON Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<io::Error> for ToolCallError {
  fn from(error: io::Error) -> Self {
    ToolCallError { message: format!("IO Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<String> for ToolCallError {
  fn from(message: String) -> Self {
    ToolCallError { message, source: None }
  }
}
