use async_openai::types::{ChatCompletionRequestMessage, CreateChatCompletionResponse};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::errors::SessionManagerError;
use crate::file_chunker::FileChunker;
use crate::gpt_connector::GPTConnector;
use crate::gpt_connector::Model;

const SESSIONS_DIR: &str = "./data/sessions";
const INGESTED_DIR: &str = "./data/ingested";

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
        Self { gpt_connector, session_data: Session::new(session_id, model ) }
    }

    // For creating from existing session data
    pub fn load_session(session_file: &str, gpt_connector: &'a GPTConnector) ->  SessionManager<'a> {
        let session_file_path = Path::new(session_file);
        let data = fs::read_to_string(session_file_path).unwrap();
        let session_data: Session = serde_json::from_str(&data).unwrap();
        Self { 
            gpt_connector, 
            session_data 
        }
    }

    pub fn save_session(&self ) -> io::Result<()> {
        let session_file_path = self.get_session_filepath();
        let data = serde_json::to_string(&self.session_data)?;
        fs::write(session_file_path, data)?;
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

    // pub fn load_session(&self, session_filename: &Path) ult<Session, io::Error> {
    //     let session_content = fs::read_to_string(session_filename)?;
    //     serde_json::from_str(&session_content)
    //         .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    // }

    pub fn load_last_session_filename() -> Option<PathBuf> {
        let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
        if last_session_path.exists() {
            Some(fs::read_to_string(last_session_path).unwrap().into())
        } else {
            None
        }
    }

    pub fn save_last_session_filename(&self) {
        Self::ensure_directory_exists(SESSIONS_DIR).unwrap();
        let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
        fs::write(last_session_path, self.get_session_filepath().display().to_string()).unwrap();
    }

    fn ensure_directory_exists(dir: &str) -> io::Result<()> {
        let dir_path = Path::new(dir);
        if !dir_path.exists() {
            fs::create_dir_all(&dir_path)?;
        }
        Ok(())
    }

    pub fn get_session_filepath(&self) -> PathBuf {
        Path::new(SESSIONS_DIR).join(format!("{}.json", self.session_data.session_id))
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
    pub async fn handle_ingest(&self, input: &String) -> Result<(), SessionManagerError> {

        // This vector will store paths that need to be processed.
        let mut paths_to_process = Vec::new();

        // Try to interpret the input as a path.
        let input_path: Result<PathBuf, std::convert::Infallible> = PathBuf::from_str(input);

        // If it's a valid path, check if it points to a directory or a file.
        if let Ok(p) = input_path {
            if p.is_dir() {
                // If it's a directory, iterate through its contents and add all the file paths to the processing list.
                for entry in fs::read_dir(&p)? {
                    let entry_path = entry?.path();
                    if entry_path.is_file() {
                        paths_to_process.push(entry_path);
                    }
                }
            } else if p.is_file() {
                // If it's a file, add it directly to the processing list.
                paths_to_process.push(p);
            }
        }

        // If the list is empty, assume the input is a block of text and treat it accordingly.
        if paths_to_process.is_empty() {
            paths_to_process.push(PathBuf::from(input));
        }

        // Iterate through all the paths to process them.
        for path in paths_to_process {
            let chunks = if path.is_file() {
                // If it's a file, chunkify its contents.
                FileChunker::chunkify_input(path.to_str().unwrap(), self.gpt_connector.model.token_limit as usize)?
            } else {
                // Otherwise, chunkify the input directly.
                FileChunker::chunkify_input(input, self.gpt_connector.model.token_limit as usize)?
            };

            // Send each chunk to the GPT API using the GPTConnector.
            let response = self.gpt_connector.send_request(chunks).await?;

            // After successful ingestion, copy the file to the 'ingested' directory.
            if path.is_file() {
                let dest_path = 
                Path::new(INGESTED_DIR)
                    .join("ingested")
                    .join(path.file_name().unwrap());
                fs::copy(&path, &dest_path)?;
            }

            for choice in &response.choices {
                println!("{:?}", choice.message.content);
            }
        }

        Ok(())
    }
}


// Tests
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_session_save_and_load() {
        let session_id = String::from("test_session");
        // let model = ;
        let mut manager = SessionManager::new(session_id, model);

        let session = Session {
            user_messages: vec![String::from("Hello"), String::from("How are you?")],
            bot_messages: vec![String::from("Hi!"), String::from("I'm good.")],
        };

        // Modify the SESSIONS_DIR to use a temporary directory for testing
        const SESSIONS_DIR: &str = "./data/sessions_test";
        manager.save_session(&session).unwrap();

        let loaded_session = manager.load_session().unwrap();
        assert_eq!(session.user_messages, loaded_session.user_messages);
        assert_eq!(session.bot_messages, loaded_session.bot_messages);

        // Clean up the temporary test directory
        let dir = Path::new(SESSIONS_DIR);
        if dir.exists() {
            fs::remove_dir_all(dir).unwrap();
        }
    }
}
