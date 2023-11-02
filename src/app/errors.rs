use async_openai::error::OpenAIError;
use std::{error::Error, fmt, io};

use crate::trace_dbg;

#[derive(Debug)]
pub enum SazidError {
  ParseError(ParseError),
  OpenAiError(OpenAIError),
  FunctionCallError(FunctionCallError),
  ConfigError(config::ConfigError),
  IoError(io::Error),
  Other(String),
  ChunkifierError(ChunkifierError),
}

impl fmt::Display for SazidError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      SazidError::ChunkifierError(err) => write!(f, "ChunkifierError: {}", err),
      SazidError::ParseError(err) => write!(f, "ParseError: {}", err),
      SazidError::ConfigError(err) => write!(f, "ConfigError: {}", err),
      SazidError::OpenAiError(err) => write!(f, "OpenAIError: {}", err),
      SazidError::FunctionCallError(err) => write!(f, "FunctionCallError: {}", err),
      SazidError::IoError(err) => write!(f, "IO error: {}", err),
      SazidError::Other(err) => write!(f, "Error: {}", err),
    }
  }
}

// Implement TryFrom with FromResidual for the custom error type
impl TryFrom<Result<(), config::ConfigError>> for SazidError {
  type Error = config::ConfigError;

  fn try_from(result: Result<(), config::ConfigError>) -> Result<Self, config::ConfigError> {
    match result {
      Ok(_) => Ok(SazidError::Other("".to_string())),
      Err(err) => Err(err),
    }
  }
}
impl TryFrom<Result<(), io::Error>> for SazidError {
  type Error = io::Error;

  fn try_from(result: Result<(), io::Error>) -> Result<Self, io::Error> {
    match result {
      Ok(_) => Ok(SazidError::Other("".to_string())),
      Err(err) => Err(err),
    }
  }
}

//│      required for `std::result::Result<app::App, app::errors::SazidError>` to implement `std::ops::FromResidual<std::result::Result<std::convert::Infallible, config::ConfigError>>`
impl From<String> for SazidError {
  fn from(message: String) -> Self {
    SazidError::Other(message)
  }
}

#[derive(Debug)]
pub struct ParseError {
  message: String,
}

impl ParseError {
  pub fn new(message: &str) -> Self {
    trace_dbg!("ParseError: {}", message);
    ParseError { message: message.to_string() }
  }
}

impl fmt::Display for ParseError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "ParseError: {}", self.message)
  }
}

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

#[derive(Debug)]
pub enum ChunkifierError {
  IO(std::io::Error),
  Utf8(std::string::FromUtf8Error),
  Other(String),
}

#[derive(Debug)]
pub enum GPTConnectorError {
  Reqwest(reqwest::Error),
  OpenAI(OpenAIError),
  APIError(OpenAIError),
  Other(String),
}

#[derive(Debug)]
pub enum SessionManagerError {
  FileChunker(ChunkifierError),
  GPTConnector(GPTConnectorError),
  PdfExtractor(PdfExtractorError),
  FileNotFound(String),
  ReadError,
  IO(std::io::Error),
  Other(String),
}

#[derive(Debug)]
pub enum PdfExtractorError {
  IO(std::io::Error),
  Other(String),
}

impl fmt::Display for ChunkifierError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      ChunkifierError::IO(err) => write!(f, "IO error: {}", err),
      ChunkifierError::Utf8(err) => write!(f, "UTF-8 conversion error: {}", err),
      ChunkifierError::Other(err) => write!(f, "Other error: {}", err),
    }
  }
}

impl fmt::Display for GPTConnectorError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      GPTConnectorError::Reqwest(err) => write!(f, "Reqwest error: {}", err),
      GPTConnectorError::OpenAI(err) => write!(f, "OpenAI error: {}", err),
      GPTConnectorError::APIError(err) => write!(f, "API error: {}", err),
      GPTConnectorError::Other(err) => write!(f, "Other error: {}", err),
    }
  }
}

impl fmt::Display for SessionManagerError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      SessionManagerError::FileChunker(err) => write!(f, "FileChunker error: {}", err),
      SessionManagerError::GPTConnector(err) => write!(f, "GPTConnector error: {}", err),
      SessionManagerError::PdfExtractor(err) => write!(f, "PdfExtractor error: {}", err),
      SessionManagerError::IO(err) => write!(f, "IO error: {}", err),
      SessionManagerError::Other(err) => write!(f, "Other error: {}", err),
      SessionManagerError::FileNotFound(file) => {
        write!(f, "Session file not found: {}", file)
      },
      SessionManagerError::ReadError => {
        write!(f, "Error reading the session file")
      },
    }
  }
}

impl fmt::Display for PdfExtractorError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      PdfExtractorError::IO(err) => write!(f, "IO error: {}", err),
      PdfExtractorError::Other(err) => write!(f, "Other error: {}", err),
    }
  }
}

impl std::error::Error for GPTConnectorError {}
impl std::error::Error for SessionManagerError {}

impl From<std::io::Error> for SessionManagerError {
  fn from(err: std::io::Error) -> SessionManagerError {
    SessionManagerError::IO(err)
  }
}

impl From<ChunkifierError> for SessionManagerError {
  fn from(err: ChunkifierError) -> SessionManagerError {
    SessionManagerError::FileChunker(err)
  }
}

impl From<GPTConnectorError> for SessionManagerError {
  fn from(err: GPTConnectorError) -> SessionManagerError {
    SessionManagerError::GPTConnector(err)
  }
}
impl From<PdfExtractorError> for SessionManagerError {
  fn from(err: PdfExtractorError) -> SessionManagerError {
    SessionManagerError::PdfExtractor(err)
  }
}

impl From<OpenAIError> for GPTConnectorError {
  fn from(err: OpenAIError) -> GPTConnectorError {
    GPTConnectorError::OpenAI(err)
  }
}

impl From<reqwest::Error> for GPTConnectorError {
  fn from(err: reqwest::Error) -> GPTConnectorError {
    GPTConnectorError::Reqwest(err)
  }
}

impl From<std::io::Error> for ChunkifierError {
  fn from(err: std::io::Error) -> ChunkifierError {
    ChunkifierError::IO(err)
  }
}

impl From<std::io::Error> for PdfExtractorError {
  fn from(err: std::io::Error) -> PdfExtractorError {
    PdfExtractorError::IO(err)
  }
}
