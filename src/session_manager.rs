use async_openai::types::{Role, CreateChatCompletionRequest, ChatChoice};
use async_openai::types::{ChatCompletionRequestMessage, CreateChatCompletionResponse};
use tiktoken_rs::model;
use tokio::runtime::Runtime;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use crate::consts::CHUNK_TOKEN_LIMIT;
use crate::errors::SessionManagerError;
use crate::types::*;
use crate::utils;

pub const SESSIONS_DIR: &str = "data/sessions";
pub const INGESTED_DIR: &str = "data/ingested";

impl Session {
    pub fn new(session_id: String, model:Model) -> Self {
        Self {
            session_id,
            model,
            interactions: Vec::new(),
        }
    }
}

impl SessionManager {
    const MAX_FUNCTION_CALL_DEPTH: u32 = 3;

    pub fn new(settings: GPTSettings, include_functions:bool, session_data: Option<Session>, rt: Runtime) -> SessionManager {
        let gpt_connector = GPTConnector::new(settings.clone(), include_functions);
        let model = rt.block_on( async {gpt_connector.select_model().await}).unwrap();
        match session_data {
            Some(session_data) => Self {
                include_functions,
                gpt_connector,
                cached_request: None,
                session_data,
                rt
            },
            None => Self {
                include_functions,
                gpt_connector,
                cached_request: None,
                session_data: Session::new(utils::generate_session_id(), model),
                rt
            }
        }
    }

    pub fn get_model(&self) -> Model {
        self.session_data.model.clone()
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

    // list all sessions in the sessions directory
    pub fn list_sessions() -> io::Result<Vec<PathBuf>> {
        utils::ensure_directory_exists(SESSIONS_DIR)?;
        let mut sessions: Vec<PathBuf> = Vec::new();
        for entry in fs::read_dir(SESSIONS_DIR)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                sessions.push(path);
            }
        }
        Ok(sessions)
    }
    
    fn get_request_messages(&self) -> Vec<ChatCompletionRequestMessage> {
        self.session_data
            .interactions
            .iter()
            .flat_map(|interaction| interaction.request.clone())
            .collect()
    }
    
    pub fn submit_input(&mut self, input: &String) -> Result<Vec<ChatChoice>, SessionManagerError> {
        let chunks = Chunkifier::parse_input(input, CHUNK_TOKEN_LIMIT as usize, self.session_data.model.token_limit as usize).unwrap();
        let previous_messages = self.get_request_messages();
        let request = self.gpt_connector.construct_request(chunks, previous_messages, self.session_data.model.clone());
        
        let response = self.rt.block_on(async { self.gpt_connector.send_request(request, Self::MAX_FUNCTION_CALL_DEPTH).await }).unwrap();
        
        self.add_interaction_for_cached_request(response.clone());
        Ok(response.choices)
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
        Chunkifier::chunkify_input(
            &input,
            self.session_data.model.token_limit as usize,
        )
        .unwrap()
    }
}

// Tests
#[cfg(test)]
mod tests {

    
}
