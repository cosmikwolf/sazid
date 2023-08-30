use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use async_openai::types::{CreateChatCompletionResponse, ChatCompletionRequestMessage};

const SESSIONS_DIR: &str = "./data/sessions";
const INGESTED_DIR: &str = "./data/ingested";

pub struct Model {
    pub endpoint: &'static str,
    pub name: &'static str,
    pub tokens_limit: usize,
}

impl Model {
    pub const GPT3: Model = Model {
        endpoint: "https://api.openai.com/v1/models/text-davinci-003/completions",
        name: "gpt-3.5-turbo",
        tokens_limit: 4096,
    };
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub user_messages: Vec<String>,
    pub bot_messages: Vec<String>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            user_messages: Vec::new(),
            bot_messages: Vec::new(),
        }
    }
}

pub struct SessionManager {
    session_id: String,
    model: Model,
}

impl SessionManager {
    pub fn new(session_id: String, model: Model) -> Self {
        Self { session_id, model }
    }

    fn ensure_directory_exists(dir: &str) -> io::Result<()> {
        let dir_path = Path::new(dir);
        if !dir_path.exists() {
            fs::create_dir_all(&dir_path)?;
        }
        Ok(())
    }

    pub fn get_session_filepath(&self) -> PathBuf {
        Path::new(SESSIONS_DIR).join(format!("{}.json", self.session_id))
    }

    pub fn get_ingested_filepath(&self) -> PathBuf {
        Path::new(INGESTED_DIR).join(format!("{}.json", self.session_id))
    }

    pub fn save_chat_to_session(
        &self,
        request: &ChatCompletionRequestMessage,
        response: &Option<CreateChatCompletionResponse>,
    ) -> io::Result<()> {
        Self::ensure_directory_exists(SESSIONS_DIR)?;

        let session_file_path = self.get_session_filepath();
        
        #[derive(Serialize)]
        struct SessionLogEntry<'a> {
            request: &'a ChatCompletionRequestMessage,
            response: &'a Option<CreateChatCompletionResponse>,
        }
        
        let log_entry = SessionLogEntry {
            request,
            response,
        };
        
        let data = serde_json::to_string(&log_entry)?;
        fs::write(session_file_path, data)?;
        Ok(())
    }

    pub fn save_ingested_file(&self, content: &str) -> io::Result<()> {
        Self::ensure_directory_exists(INGESTED_DIR)?;
        
        let ingested_file_path = self.get_ingested_filepath();
        fs::write(ingested_file_path, content)?;
        Ok(())
    }

    pub fn load_session(&self) -> io::Result<Session> {
        let session_file_path = self.get_session_filepath();
        let data = fs::read_to_string(session_file_path)?;
        let session: Session = serde_json::from_str(&data)?;
        Ok(session)
    }

    pub fn save_session(&self, session: &Session) -> io::Result<()> {
        let session_file_path = self.get_session_filepath();
        let data = serde_json::to_string(session)?;
        fs::write(session_file_path, data)?;
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
        let model = Model::GPT3;
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
