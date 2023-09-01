use async_openai::types::Role;
use async_openai::types::{ChatCompletionRequestMessage, CreateChatCompletionResponse};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::chunkifier::Chunkifier;
use crate::errors::SessionManagerError;
use crate::gpt_connector::GPTConnector;
use crate::gpt_connector::Model;
use crate::ui;
use crate::utils;

pub const SESSIONS_DIR: &str = "data/sessions";
pub const INGESTED_DIR: &str = "data/ingested";


#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub model: Model,
    pub requests: Vec<ChatCompletionRequestMessage>,
    pub responses: Vec<CreateChatCompletionResponse>,
}

impl Session {
    pub fn new(session_id: String, model: Model) -> Self {
        Self {
            session_id,
            model,
            requests: Vec::new(),
            responses: Vec::new(),
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
pub struct SessionManager<'a> {
    gpt_connector: &'a GPTConnector,
    pub session_data: Session,
}

impl<'a> SessionManager<'a> {
    pub fn new(session_id: String, gpt_connector: &'a GPTConnector) -> SessionManager<'a> {
        let model = gpt_connector.model.clone();
        Self {
            gpt_connector,
            session_data: Session::new(session_id, model),
        }
    }

    // load a session from a file
    pub fn load_session_from_file(
        session_file_path: PathBuf,
        gpt_connector: &'a GPTConnector,
    ) -> SessionManager<'a> {
        if !session_file_path.exists(){
            ui::UI::display_error_message(format!("Session file not found: {}", session_file_path.display()));
            return SessionManager::new(utils::generate_session_id(), gpt_connector);
        } else {
            let data = fs::read_to_string(session_file_path).unwrap();
            let session_data: Session = serde_json::from_str(&data).unwrap();
    
            Self {
                gpt_connector,
                session_data,
            }
        }
    }

    pub fn save_session(&self) -> io::Result<()> {
        let session_file_path = self.get_session_filepath();
        let data = serde_json::to_string(&self.session_data)?;
        fs::write(session_file_path, data)?;
        self.save_last_session_file_path();
        Ok(())
    }
    pub fn get_requests(&self) -> &Vec<ChatCompletionRequestMessage> {
        &self.session_data.requests
    }

    pub fn get_responses(&self) -> &Vec<CreateChatCompletionResponse> {
        &self.session_data.responses
    }

    pub fn add_request(&mut self, request: ChatCompletionRequestMessage) {
        self.session_data.requests.push(request);
    }

    pub fn add_response(&mut self, response: CreateChatCompletionResponse) {
        self.session_data.responses.push(response);
    }

    pub fn load_last_session_file_path() -> Option<PathBuf> {
        let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
        if last_session_path.exists() {
            Some(fs::read_to_string(last_session_path).unwrap().into())
        } else {
            None
        }
    }

    pub fn save_last_session_file_path(&self) {
        Self::ensure_directory_exists(SESSIONS_DIR).unwrap();
        let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
        fs::write(
            last_session_path,
            self.get_session_filepath().display().to_string(),
        )
        .unwrap();
    }

    fn ensure_directory_exists(dir: &str) -> io::Result<()> {
        let dir_path = Path::new(dir);
        if !dir_path.exists() {
            fs::create_dir_all(&dir_path)?;
        }
        Ok(())
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
        Self::ensure_directory_exists(INGESTED_DIR)?;

        let ingested_file_path = self.get_ingested_filepath();
        fs::write(ingested_file_path, content)?;
        Ok(())
    }

    /// This function takes in an input which could be a path to a directory, a path to a file,
    /// a block of text, or a URL. Depending on the type of input, it processes (or ingests) the
    /// content by converting it into chunks of text and then sends each chunk to the GPT API.
    pub async fn handle_ingest(&mut self, input: &String) -> Result<(), SessionManagerError> {
        let chunks =
            Chunkifier::chunkify_input(input, self.gpt_connector.model.token_limit as usize)
                .unwrap();
        // Send each chunk to the GPT API using the GPTConnector.
        let response = self.gpt_connector.send_request(
            self.gpt_connector
                .construct_request_message_array(Role::User, chunks),
        ).await?;
        // After successful ingestion, copy the file to the 'ingested' directory.
        Ok(self.add_response(response))
    }
}

// Tests
#[cfg(test)]
mod tests {
    use crate::gpt_connector::GPTSettings;

    use super::*;

    use tempfile::tempdir;
    #[tokio::test]
    async fn test_session_save_and_load() {
        let temp_dir = tempdir().unwrap();
        let settings: GPTSettings =
            toml::from_str(std::fs::read_to_string("Settings.toml").unwrap().as_str()).unwrap();
        let gpt: GPTConnector = GPTConnector::new(&settings).await;
        let session_id = crate::utils::generate_session_id();
        let mut session = SessionManager::new(session_id, &gpt);

        session.add_request(ChatCompletionRequestMessage {
            role: async_openai::types::Role::User,
            content: Some(String::from("Hello")),
            name: Some("user".to_string()),
            function_call: None,
        });
        session.add_request(ChatCompletionRequestMessage {
            role: async_openai::types::Role::User,
            content: Some(String::from("How are you?")),
            name: Some("user".to_string()),
            function_call: None,
        });

        // Modify the SESSIONS_DIR to use a temporary directory for testing
        const SESSIONS_DIR: &str = "./data/sessions_test";
        session.save_session().unwrap();

        let loaded_session =
            SessionManager::load_session_from_file(session.get_session_filepath(), &gpt);
        assert_eq!(
            session.session_data.requests,
            loaded_session.session_data.requests
        );
        assert_eq!(
            session.session_data.responses,
            loaded_session.session_data.responses
        );

        // Clean up the temporary test directory
        let dir = Path::new(SESSIONS_DIR);
        if dir.exists() {
            fs::remove_dir_all(dir).unwrap();
        }
    }
}
