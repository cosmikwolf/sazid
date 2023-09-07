use std::{path::PathBuf, collections::BTreeMap};

use async_openai::{types::{Role, ChatCompletionRequestMessage, CreateChatCompletionResponse}, config::OpenAIConfig, Client};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;

// GPT Connector types
#[derive(Debug, Deserialize, Clone)]
pub struct GPTSettings {
    pub default: ModelConfig,
    pub fallback: ModelConfig,
    pub load_session: Option<String>,
    pub save_session: Option<String>,
}
#[derive(Debug, Deserialize, Clone)]
pub struct ModelConfig {
    pub name: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Model {
    pub(crate) name: String,
    pub(crate) endpoint: String,
    pub token_limit: u32,
}

pub struct ModelsList {
    pub default: Model,
    pub fallback: Model,
}
#[derive(Clone)]
pub struct GPTConnector {
    pub client: Client<OpenAIConfig>,
    pub model: Model,
}

pub struct GPTResponse {
    pub role: Role,
    pub content: String,
}

// PDF Parser types
pub struct PdfText {
    pub text: BTreeMap<u32, Vec<String>>, // Key is page number
    pub errors: Vec<String>,
}

// Session Manager types

#[derive(Debug, Serialize, Deserialize)]
#[derive(Clone)]
pub struct ChatInteraction {
    pub request: Vec<ChatCompletionRequestMessage>,
    pub response: CreateChatCompletionResponse,
}


#[derive(Debug, Serialize, Deserialize)]
#[derive(Clone)]
pub struct Session {
    pub session_id: String,
    pub model: Model,
    pub interactions: Vec<ChatInteraction>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IngestedData {
    session_id: String,
    file_path: String,
    chunk_num: u32,
    content: String,
}
pub struct SessionManager {
    pub gpt_connector: GPTConnector,
    pub cached_request: Option<Vec<ChatCompletionRequestMessage>>,
    pub session_data: Session,
    pub rt: Runtime,
}
pub struct Message {
    pub role: Role,
    pub content: String,
}

// chunkifier types

pub struct UrlData {
    urls: String,
    data: String
}
pub struct FilePathData {
    file_paths: String,
    data: String
}
pub struct IngestData {
    pub text: String,
    pub urls: Vec<String>,
    pub file_paths: Vec<PathBuf>
}
pub struct Chunkifier {}

// a display function for Message
impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format_chat_message(f, self.role.clone(), self.content.clone())
    }
}

fn format_chat_message(f: &mut std::fmt::Formatter<'_>, role: Role, message: String) -> std::fmt::Result {
    match role {
        Role::User => write!(f, "You: {}\n\r", message),
        Role::Assistant => write!(f, "GPT: {}\n\r", message),
        _ => Ok(()),
    }
}

