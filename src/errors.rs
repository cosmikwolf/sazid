use async_openai::error::OpenAIError;
use std::fmt;

#[derive(Debug)]
pub enum FileChunkerError {
    /// Errors related to Input/Output operations, e.g., file reading/writing.
    IO(std::io::Error),
    /// A generic error type to capture any other miscellaneous errors.
    /// Errors that arise when trying to convert bytes into a UTF-8 string.
    Utf8(std::string::FromUtf8Error),
    /// A generic error type to capture any other miscellaneous errors.
    Other(String),
}

#[derive(Debug)]
pub enum GPTConnectorError {
    /// Errors originating from the `reqwest` crate, used for HTTP requests.
    Reqwest(reqwest::Error),
    /// Errors specific to the `OpenAI` API.
    OpenAI(OpenAIError),
    /// Errors related to the GPT API responses.
    APIError(String),
    /// A generic error type to capture any other miscellaneous errors.
    Other(String),
}

#[derive(Debug)]
pub enum SessionManagerError {
    /// Errors propagated from the `FileChunker` module.
    FileChunker(FileChunkerError),
    /// Errors propagated from the `GPTConnector` module.
    GPTConnector(GPTConnectorError),
    /// Errors propagated from the `PdfExtractor` module.
    PdfExtractor(PdfExtractorError),
    /// Errors that arise when a session file is not found.
    FileNotFound(String),
    /// Errors related to reading a session file.
    ReadError,
    /// Errors related to Input/Output operations.
    IO(std::io::Error),
    /// A generic error type to capture any other miscellaneous errors.
    /// Errors that arise when trying to convert bytes into a UTF-8 string.
    Other(String),
}

#[derive(Debug)]
pub enum PdfExtractorError {
    /// Errors related to Input/Output operations during PDF extraction.
    IO(std::io::Error),
    /// A generic error type to capture any other miscellaneous errors.
    /// Errors that arise when trying to convert bytes into a UTF-8 string.
    Other(String),
}

impl fmt::Display for FileChunkerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FileChunkerError::IO(err) => write!(f, "IO error: {}", err),
            FileChunkerError::Utf8(err) => write!(f, "UTF-8 conversion error: {}", err),
            FileChunkerError::Other(err) => write!(f, "Other error: {}", err),
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
            SessionManagerError::FileNotFound(file) => { write!(f, "Session file not found: {}", file) },
            SessionManagerError::ReadError => { write!(f, "Error reading the session file") },
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

impl From<FileChunkerError> for SessionManagerError {
    fn from(err: FileChunkerError) -> SessionManagerError {
        SessionManagerError::FileChunker(err)
    }
}

impl From<GPTConnectorError> for SessionManagerError {
    fn from(err: GPTConnectorError) -> SessionManagerError {
        SessionManagerError::GPTConnector(err)
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

impl From<std::io::Error> for FileChunkerError {
    fn from(err: std::io::Error) -> FileChunkerError {
        FileChunkerError::IO(err)
    }
}

impl From<std::io::Error> for PdfExtractorError {
    fn from(err: std::io::Error) -> PdfExtractorError {
        PdfExtractorError::IO(err)
    }
}
