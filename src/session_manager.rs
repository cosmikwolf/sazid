use crate::gpt_connector::ChatCompletionRequestMessage;

use serde_json;
use std::fs;
use chrono::Local;
use std::path::Path;

pub struct SessionManager;

impl SessionManager {
    // Ensure the session_data directory exists.
    fn ensure_session_data_directory_exists() {
        let path = Path::new("session_data");
        if !path.exists() {
            fs::create_dir(path).expect("Failed to create session_data directory");
        }
    }

    // Generate a new session filename based on the current date and time.
    pub fn new_session_filename() -> String {
        Local::now().format("%Y-%m-%d_%H-%M-%S.json").to_string()  // Adjusted to include seconds
    }

    // Load a session from a given filename.
    pub fn load_session(filename: &str) -> Result<Vec<ChatCompletionRequestMessage>, std::io::Error> {
        Self::ensure_session_data_directory_exists();  // Ensure directory exists before reading
        let data = fs::read(format!("session_data/{}", filename))?;
        let messages = serde_json::from_slice(&data).unwrap_or_default();
        Ok(messages)
    }

    // Save a session to a given filename.
    pub fn save_session(filename: &str, messages: &Vec<ChatCompletionRequestMessage>) -> Result<(), std::io::Error> {
        Self::ensure_session_data_directory_exists();  // Ensure directory exists before writing
        let data = serde_json::to_vec(messages)?;
        fs::write(format!("session_data/{}", filename), data)?;
        Ok(())
    }

    // Load the last used session filename.
    pub fn load_last_session_filename() -> Option<String> {
        Self::ensure_session_data_directory_exists();  // Ensure directory exists before reading
        if let Ok(filename) = fs::read_to_string("session_data/last_session.txt") {
            return Some(filename);
        }
        None
    }

    // Save the last used session filename.
    pub fn save_last_session_filename(filename: &str) -> Result<(), std::io::Error> {
        Self::ensure_session_data_directory_exists();  // Ensure directory exists before writing
        fs::write("session_data/last_session.txt", filename)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_openai::types::Role;

    #[test]
    fn test_session_management() {
        // Test session filename generation
        let filename = SessionManager::new_session_filename();
        assert!(filename.contains("-"));  // Check if filename contains date delimiters

        // Test session saving and loading
        let messages = vec![ChatCompletionRequestMessage {
            role: Role::User,
            content: "Test message".to_string(),
        }];
        SessionManager::save_session(&filename, &messages).unwrap();
        let loaded_messages = SessionManager::load_session(&filename).unwrap();
        assert_eq!(messages, loaded_messages);

        // Test last session filename saving and loading
        SessionManager::save_last_session_filename(&filename).unwrap();
        let last_session_filename = SessionManager::load_last_session_filename().unwrap();
        assert_eq!(filename, last_session_filename);
    }
}
