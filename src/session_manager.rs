use async_openai::types::{Role, CreateChatCompletionRequest, ChatChoice};
use async_openai::types::{ChatCompletionRequestMessage, CreateChatCompletionResponse};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use crate::errors::SessionManagerError;
use crate::gpt_connector::{GPTConnector, GPTSettings};
use crate::gpt_connector::Model;
// use crate::ui::UI;
use crate::utils;
use crate::chunkifier::Chunkifier;
use crate::types::Message;

pub const SESSIONS_DIR: &str = "data/sessions";
pub const INGESTED_DIR: &str = "data/ingested";

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

impl Session {
    pub fn new(session_id: String, model: Model) -> Self {
        Self {
            session_id,
            model,
            interactions: Vec::new(),
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct IngestedData {
    session_id: String,
    file_path: String,
    chunk_num: u32,
    content: String,
}
pub struct SessionManager {
    gpt_connector: GPTConnector,
    cached_request: Option<Vec<ChatCompletionRequestMessage>>,
    pub session_data: Session,
    rt: Runtime,
}

impl SessionManager {
    pub fn new(settings: GPTSettings, session_data: Option<Session>, rt: Runtime) -> SessionManager {
        let gpt_connector = rt.block_on(async { GPTConnector::new(&settings).await});
        let model = gpt_connector.model.clone();
        match session_data {
            Some(session_data) => Self {
                gpt_connector,
                cached_request: None,
                session_data,
                rt
            },
            None => Self {
                gpt_connector,
                cached_request: None,
                session_data: Session::new(utils::generate_session_id(), model),
                rt
            }
        }
    }

    pub fn load_session_data(&mut self, session_data: Session) -> io::Result<()> {
        self.session_data = session_data;
        Ok(())
    }

    pub fn save_session(&self) -> io::Result<()> {
        utils::ensure_directory_exists(SESSIONS_DIR).unwrap();
        let session_file_path = self.get_session_filepath();
        let data = serde_json::to_string(&self.session_data)?;
        fs::write(session_file_path, data)?;
        self.save_last_session_file_path();
        Ok(())
    }

    // Get the responses from the session data
    pub fn get_responses(&self) -> Vec<CreateChatCompletionResponse> {
        self.session_data
            .interactions
            .iter()
            .map(|interaction| interaction.response.clone())
            .collect()
    }

    // Get the chat history from the session data
    pub fn get_chat_history(&self) -> Vec<(Role, String)> {
        let mut chat_history: Vec<(Role, String)> = Vec::new();
        for interaction in &self.session_data.interactions {
            for request in self.get_request_messages() {
                chat_history.push((request.role.clone(), request.content.clone().unwrap_or_default()));
            }
            for choice in &interaction.response.choices {
                chat_history.push((choice.message.role.clone(), choice.message.content.clone().unwrap_or_default()));
            }
        }
        chat_history
    }
    // Add an interaction to the session data
    pub fn add_interaction(
        &mut self,
        request: Vec<ChatCompletionRequestMessage>,
        response: CreateChatCompletionResponse,
    ) {
        self.session_data
            .interactions
            .push(ChatInteraction { request, response })
    }

    // a function that will iterate through all the interactions 
    // and return a Vec<Message> that contains all the messages
    // in both the request and response
    pub fn get_messages(&self) -> Vec<Message> {
        let mut messages: Vec<Message> = Vec::new();
        for interaction in &self.session_data.interactions {
            for request in &interaction.request {
                messages.push(Message {
                    role: request.role.clone(),
                    content: request.content.clone().unwrap_or_default(),
                });
            }
            for choice in &interaction.response.choices {
                messages.push(Message {
                    role: choice.message.role.clone(),
                    content: choice.message.content.clone().unwrap_or_default(),
                });
            }
        }
        messages
    }

    pub fn add_interaction_for_cached_request(
        &mut self,
        response: CreateChatCompletionResponse,
    ) {
        if let Some(request) = self.cached_request.clone() {
            self.add_interaction(request, response);
            self.cached_request = None;
        }
    }

    pub fn get_last_session_file_path() -> Option<PathBuf> {
        utils::ensure_directory_exists(SESSIONS_DIR).unwrap();
        let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
        if last_session_path.exists() {
            Some(fs::read_to_string(last_session_path).unwrap().into())
        } else {
            None
        }
    }

    fn get_request_messages(&self) -> Vec<ChatCompletionRequestMessage> {
        self.session_data
            .interactions
            .iter()
            .map(|interaction| interaction.request.clone())
            .flatten()
            .collect()
    }
    
    pub fn submit_input(&mut self, input: &String) -> Result<Vec<ChatChoice>, SessionManagerError> {
        let chunks = Chunkifier::parse_input(&input, self.gpt_connector.model.token_limit as usize).unwrap();
        let request = self.construct_request_and_cache(chunks);
        // Send each chunk to the GPT API using the GPTConnector.
        
        let response = self.rt.block_on(async { self.gpt_connector.send_request(request).await }).unwrap();
        self.add_interaction_for_cached_request(response.clone());
        Ok(response.choices)
    }
    
    pub fn construct_request_and_cache(&mut self, content: Vec<String> ) -> CreateChatCompletionRequest {
        // iterate through the vector of ChatCompletionRequestMessage from the interactions stored in session_data as a clone
        let mut messages = self.get_request_messages();

        let mut new_messages: Vec<ChatCompletionRequestMessage> = Vec::new();
        for item in content {
            let message = ChatCompletionRequestMessage {
                role: Role::User,
                content: Some(item),
                ..Default::default()
            };
            messages.push(message.clone());
            new_messages.push(message);
        }

        // cache the request so it can be stored in the session data
        self.cached_request = Some(new_messages);

        // return a new CreateChatCompletionRequest
        CreateChatCompletionRequest {
            model: self.gpt_connector.model.name.clone(),
            messages,
            ..Default::default()
        }
    }

    pub fn save_last_session_file_path(&self) {
        utils::ensure_directory_exists(SESSIONS_DIR).unwrap();
        let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
        fs::write(
            last_session_path,
            self.get_session_filepath().display().to_string(),
        )
        .unwrap();
    }

    pub fn get_session_filepath(&self) -> PathBuf {
        Path::new(SESSIONS_DIR).join(self.get_session_filename())
    }

    pub fn get_session_filename(&self) -> String {
        format!("{}.json", self.session_data.session_id)
    }

    pub fn get_ingested_filepath(&self) -> PathBuf {
        Path::new(INGESTED_DIR).join(format!("{}.json", self.session_data.session_id))
    }

    pub fn save_ingested_file(&self, content: &str) -> io::Result<()> {
        utils::ensure_directory_exists(INGESTED_DIR)?;

        let ingested_file_path = self.get_ingested_filepath();
        fs::write(ingested_file_path, content)?;
        Ok(())
    }

    // a function that takes an a string input,
    // it will chunkify with Chunkifier::chunkify_input and return a vector of strings
    pub fn parse_input(&self, input:String) -> Vec<String> {
        let chunks = Chunkifier::chunkify_input(
            &input,
            self.session_data.model.token_limit as usize,
        )
        .unwrap();
        chunks
    }
    
    
    /// This function takes in an input which could be a path to a directory, a path to a file,
    /// a block of text, or a URL. Depending on the type of input, it processes (or ingests) the
    /// content by converting it into chunks of text and then sends each chunk to the GPT API.
    pub async fn handle_ingest(&mut self, chunks: Vec<String>) -> Result<(), SessionManagerError> {
        let request = self.construct_request_and_cache(chunks);
        // Send each chunk to the GPT API using the GPTConnector.
        let response = self.gpt_connector.send_request(request.clone()).await?;
        // After successful ingestion, copy the file to the 'ingested' directory.
        Ok(self.add_interaction_for_cached_request(response.clone()))
    }
}

// Tests
#[cfg(test)]
mod tests {

    
}
