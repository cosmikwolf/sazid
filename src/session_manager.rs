use crate::gpt_connector::ChatCompletionRequestMessage;
use chrono::Local;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde_json;
use std::fs;
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

    // Generate a new session filename based on the current date, time, and a random 16-bit hash.
    pub fn new_session_filename() -> String {
        let current_time = Local::now().format("%Y%m%d%H%M").to_string();
        let random_hash: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .map(|b| b as char)
            .take(4)
            .collect();
        let filename = format!("{}_{}.json", current_time, random_hash);
        filename  // Return the filename
    }

    // Load a session from a given filename.
    pub fn load_session(
        filename: &str,
    ) -> Result<Vec<ChatCompletionRequestMessage>, std::io::Error> {
        Self::ensure_session_data_directory_exists(); // Ensure directory exists before reading
        let data = fs::read(format!("session_data/{}", filename))?;
        let messages = serde_json::from_slice(&data).unwrap_or_default();
        Ok(messages)
    }

    // Save a session to a given filename.
    pub fn save_session(
        filename: &str,
        messages: &Vec<ChatCompletionRequestMessage>,
    ) -> Result<(), std::io::Error> {
        Self::ensure_session_data_directory_exists(); // Ensure directory exists before writing
        let data = serde_json::to_vec(messages)?;
        fs::write(format!("session_data/{}", filename), data)?;
        Ok(())
    }

    // Load the last used session filename.
    pub fn load_last_session_filename() -> Option<String> {
        Self::ensure_session_data_directory_exists(); // Ensure directory exists before reading
        if let Ok(filename) = fs::read_to_string("session_data/last_session.txt") {
            return Some(filename);
        }
        None
    }

    // Save the last used session filename.
    pub fn save_last_session_filename(filename: &str) -> Result<(), std::io::Error> {
        Self::ensure_session_data_directory_exists(); // Ensure directory exists before writing
        fs::write("session_data/last_session.txt", filename)?;
        Ok(())
    }

    // Delete a session.
    pub fn delete_session(filename: &str) -> Result<(), std::io::Error> {
        Self::ensure_session_data_directory_exists(); // Ensure directory exists before deletion
        let path = format!("session_data/{}", filename);
        if Path::new(&path).exists() {
            fs::remove_file(path)?;
        }
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
        assert!(filename.contains("_")); // Check if filename contains date delimiters

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
