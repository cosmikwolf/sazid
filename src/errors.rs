use async_openai::error::OpenAIError;
use std::fmt;

#[derive(Debug)]
pub enum FileChunkerError {
    IO(std::io::Error),
    Utf8(std::string::FromUtf8Error),
    Other(String),
}

#[derive(Debug)]
pub enum GPTConnectorError {
    Reqwest(reqwest::Error),
    OpenAI(OpenAIError),
    APIError(String),
    Other(String),
}

#[derive(Debug)]
pub enum SessionManagerError {
    FileChunker(FileChunkerError),
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
