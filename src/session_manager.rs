use crate::gpt_connector::ChatCompletionRequestMessage;
use crate::gpt_connector::GPTConnector;
use crate::gpt_connector::Role;
use crate::file_chunker;
use chrono::Local;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde_json;
use std::fs;
use std::path::PathBuf;

pub struct SessionManager {
    base_dir: PathBuf,
}

impl SessionManager {
    // Create a new SessionManager with a specified base directory.
    pub fn new(base_dir: PathBuf) -> Self {
        SessionManager { base_dir }
    }

    // Ensure the session_data directory exists.
    fn ensure_session_data_directory_exists(&self) {
        let path = self.base_dir.join("session_data");
        if !path.exists() {
            fs::create_dir(&path).expect("Failed to create session_data directory");
        }
    }

    // Generate a new session filename based on the current date, time, and a random 16-bit hash.
    pub fn new_session_filename(&self) -> String {
        let current_time = Local::now().format("%Y%m%d%H%M").to_string();
        let random_hash: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .map(|b| b as char)
            .take(4)
            .collect();
        let filename = format!("{}_{}.json", current_time, random_hash);
        filename
    }

    // Load a session from a given filename.
    pub fn load_session(&self, filename: &str) -> Result<Vec<ChatCompletionRequestMessage>, std::io::Error> {
        self.ensure_session_data_directory_exists();
        let data = fs::read(self.base_dir.join("session_data").join(filename))?;
        let messages = serde_json::from_slice(&data).unwrap_or_default();
        Ok(messages)
    }

    // Save a session to a given filename.
    pub fn save_session(&self, filename: &str, messages: &Vec<ChatCompletionRequestMessage>) -> Result<(), std::io::Error> {
        self.ensure_session_data_directory_exists();
        let data = serde_json::to_vec(messages)?;
        fs::write(self.base_dir.join("session_data").join(filename), data)?;
        Ok(())
    }

    // Load the last used session filename.
    pub fn load_last_session_filename(&self) -> Option<String> {
        self.ensure_session_data_directory_exists();
        if let Ok(filename) = fs::read_to_string(self.base_dir.join("session_data/last_session.txt")) {
            return Some(filename);
        }
        None
    }

    // Save the last used session filename.
    pub fn save_last_session_filename(&self, filename: &str) -> Result<(), std::io::Error> {
        self.ensure_session_data_directory_exists();
        fs::write(self.base_dir.join("session_data/last_session.txt"), filename)?;
        Ok(())
    }

    // Delete a session.
    pub fn delete_session(&self, filename: &str) -> Result<(), std::io::Error> {
        self.ensure_session_data_directory_exists();
        let path = self.base_dir.join("session_data").join(filename);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

pub fn handle_import(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let gpt = GPTConnector::new();
    let paths = if path.is_dir() {
        fs::read_dir(path)?
            .map(|res| res.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![path.clone()]
    };

    for path in paths {
        let mut index = 1;
        loop {
            let (chunk, _total_chunks) = file_chunker::chunk_file(&path, index);
            if chunk == "Index out of bounds." {
                break;
            }

            let user_message = ChatCompletionRequestMessage {
                role: Role::User,
                content: chunk,
            };

            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(gpt.send_request(vec![user_message]))?;

            index += 1;
        }
    }

    Ok(())

}

#[cfg(test)]
mod tests {
    use super::*;
    use async_openai::types::Role;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_session_management() {
        let manager = SessionManager::new(PathBuf::from("./"));

        // Test session filename generation
        let filename = manager.new_session_filename();
        assert!(filename.contains("_"));

        // Test session saving and loading
        let messages = vec![ChatCompletionRequestMessage {
            role: Role::User,
            content: "Test message".to_string(),
        }];
        manager.save_session(&filename, &messages).unwrap();
        let loaded_messages = manager.load_session(&filename).unwrap();
        assert_eq!(messages, loaded_messages);

        // Test last session filename saving and loading
        manager.save_last_session_filename(&filename).unwrap();
        let last_session_filename = manager.load_last_session_filename().unwrap();
        assert_eq!(filename, last_session_filename);
    }

    #[test]
    fn test_handle_import() {
        let pdf_path = Path::new("tests/data/PDF32000_2008.pdf");
        let txt_path = Path::new("tests/data/PDF32000_2008.txt");
    
        assert!(pdf_path.exists(), "PDF file does not exist");
        assert!(txt_path.exists(), "TXT file does not exist");
    }
    
}
