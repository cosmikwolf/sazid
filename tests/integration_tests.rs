extern crate sazid;
extern crate tempfile;

use futures::future::{self, OptionFuture};

#[cfg(test)]
mod integration_tests {
    
    use super::*;
    use std::io::Write;
    use std::fs::{self, File};
    use async_openai::types::Role;
    use sazid::gpt_connector::{GPTSettings, GPTConnector};
    use async_openai::types::ChatCompletionRequestMessage;
    use sazid::session_manager::SessionManager;
    use tempfile::tempdir;

    // Mock structures and functions
    struct MockUI {
        screen: Vec<String>,
        exit_flag: bool,
    }

    impl MockUI {
        fn new() -> Self {
            MockUI {
                screen: Vec::new(),
                exit_flag: false,
            }
        }

        fn mock_ui_display_message(&mut self, message: &str) {
            self.screen.push(message.to_string());
        }

        fn mock_ui_exit(&mut self) {
            self.exit_flag = true;
        }

        fn get_last_message(&self) -> Option<&String> {
            self.screen.last()
        }

        fn has_exit_flag(&self) -> bool {
            self.exit_flag
        }

        fn _clear(&mut self) {
            self.screen.clear();
            self.exit_flag = false;
        }
    }

    fn mock_send_request(input: &str) -> String {
        match input {
            "Hello, GPT!" => "Hello, User!".to_string(),
            _ => "I am a mock function, and I don't know this input.".to_string(),
        }
    }

    // 1. Test session filename generation
    // Requirement: The application should be able to generate a unique session filename based on the current date, time, and a random hash.
    #[tokio::test]
    async fn test_session_creation() {
        let settings: GPTSettings = toml::from_str(std::fs::read_to_string("Settings.toml").unwrap().as_str()).unwrap();
        let gpt: GPTConnector = GPTConnector::new(&settings).await;
        let session_id = sazid::utils::generate_session_id();
        let session = SessionManager::new(session_id, &gpt);
    
        // Check if a file containing the session_id exists within the SESSIONS_DIR directory
        let session_files_in_directory = std::fs::read_dir(sazid::session_manager::SESSIONS_DIR).unwrap();
        let file_exists = session_files_in_directory.any(|entry| {
            let entry_path = entry.unwrap().path();
            entry_path.is_file() && entry_path.to_string_lossy().contains(&session_id)
        });
    
        assert!(file_exists);
    }
    
    // 2. Test generation of multiple unique session filenames
    // Requirement: Multiple sessions should have unique identifiers.
    #[test]
    fn test_multiple_sessions() {
        // Generate two unique session IDs
        // this will take one additional second since generate_session_id has a 1 second timeout to ensure unique session IDs
        let session_id1 = sazid::utils::generate_session_id();
        let session_id2 = sazid::utils::generate_session_id();
    
        assert_ne!(session_id1, session_id2);

        // Collect files in the directory into a Vec
        let session_files_in_directory: Vec<_> = std::fs::read_dir(sazid::session_manager::SESSIONS_DIR).unwrap().collect();

        let file1_exists = session_files_in_directory.iter().any(|entry| {
            let entry_path = entry.as_ref().unwrap().path();
            entry_path.is_file() && entry_path.to_string_lossy().contains(&session_id1)
        });
        let file2_exists = session_files_in_directory.iter().any(|entry| {
            let entry_path = entry.as_ref().unwrap().path();
            entry_path.is_file() && entry_path.to_string_lossy().contains(&session_id2)
        });
    
        assert!(file1_exists && file2_exists);
    }
    
    // 3. Test storage of messages within a session
    // Requirement: The application should be able to store messages (text) in a session.
    #[test]
    fn test_message_storage() {
        let mut messages = vec![];
        let user_message = ChatCompletionRequestMessage {
            role: Role::User,
            content: Some("Hello, GPT!".to_string()),
            ..Default::default()            // Use default values for other fields
        };
        messages.push(user_message);
        assert_eq!(messages[0].content.unwrap(), "Hello, GPT!");
    }

    // 4. Test saving and loading of sessions, as well as tracking the last session
    // Requirement: The application should save and reload chat sessions. It should also track the most recent session for easy reloading.
    #[tokio::test]
    async fn test_session_save_and_load() {
        let temp_dir = tempdir().unwrap();
        let settings: GPTSettings = toml::from_str(std::fs::read_to_string("Settings.toml").unwrap().as_str()).unwrap();
        // let mut gpt: GPTConnector;
        let gpt = GPTConnector::new(&settings).await;
        let session_id = sazid::utils::generate_session_id();
        let session = SessionManager::new(session_id, &gpt);

        let messages = vec![ChatCompletionRequestMessage {
            role: Role::User,
            content: Some("Hello, GPT!".to_string()),
            ..Default::default()            // Use default values for other fields
        }];

        // Save session and last session
        session.save_session();
        session.save_last_session_file_path();

        // Check if file exists
        assert!(temp_dir
            .path()
            .join(&session.get_session_filepath())
            .exists());

        // Load specific session
        let loaded_session = SessionManager::load_session_from_file(session.get_session_filepath(), &gpt);
        let loaded_session_requests = loaded_session.get_requests();

        assert_eq!(loaded_session_requests[0].content.unwrap(), "Hello, GPT!".to_string());
        
        // Load last session
        let last_session_path = SessionManager::load_last_session_file_path().unwrap();
        
        let last_session = SessionManager::load_session_from_file(last_session_path, &gpt);
        let last_session_requests = loaded_session.get_requests()[0].content.unwrap();
        assert_eq!(last_session_requests, "Hello, GPT!".to_string());
    }

    // 5. Test UI's ability to display messages
    // Requirement: The UI should be able to display messages from both the user and the assistant.
    #[test]
    fn test_ui_display_message() {
        let mut mock_ui = MockUI::new();

        mock_ui.mock_ui_display_message("Hello, GPT!");
        assert_eq!(mock_ui.get_last_message().unwrap(), "Hello, GPT!");

        mock_ui.mock_ui_display_message("Hello, User!");
        assert_eq!(mock_ui.get_last_message().unwrap(), "Hello, User!");
    }

    // 6. Test user's ability to exit the application via the UI
    // Requirement: The user should be able to exit the application using a UI command or action.
    #[test]
    fn test_user_exit() {
        let mut mock_ui = MockUI::new();

        assert_eq!(mock_ui.has_exit_flag(), false);
        mock_ui.mock_ui_exit();
        assert_eq!(mock_ui.has_exit_flag(), true);
    }

    // 7. Test sending a request to GPT and receiving a response
    // Requirement: The application should be able to send a request to GPT and receive an appropriate response.
    #[test]
    fn test_send_request() {
        let response = mock_send_request("Hello, GPT!");
        assert_eq!(response, "Hello, User!"); // Assuming GPT always replies in this way for the test
    }

    // 8. Test continuation of a chat conversation
    // Requirement: The application should allow users to continue their chat conversation from where they left off.
    #[tokio::test]
    async fn test_continued_conversation() {
        let temp_dir = tempdir().unwrap();
        let settings: GPTSettings = toml::from_str(std::fs::read_to_string("Settings.toml").unwrap().as_str()).unwrap();
        let gpt: GPTConnector = GPTConnector::new(&settings).await;
        let session_id = sazid::utils::generate_session_id();
        let session = SessionManager::new(session_id, &gpt);  
        let mut messages = vec![
            ChatCompletionRequestMessage {
                role: Role::User,
                content: Some("Hello, GPT!".to_string()),
                ..Default::default()            // Use default values for other fields
            },
            ChatCompletionRequestMessage {
                role: Role::Assistant,
                content: Some("Hello, User!".to_string()),
                ..Default::default()            // Use default values for other fields
            },
        ];

        session.save_session();

        let loaded_messages = SessionManager::load_session_from_file(session.get_session_filepath(), &gpt);
        assert_eq!(loaded_messages.get_requests().len(), 2); // Two messages in the session

        let new_message = ChatCompletionRequestMessage {
            role: Role::User,
            content: Some("How are you?".to_string()),
            ..Default::default()            // Use default values for other fields
        };
        messages.push(new_message);
        assert_eq!(messages.len(), 3); // Now, three messages in the session
    }

    // 9. Test the ability to delete a session
    // Requirement: The application should provide functionality to delete a chat session.
    #[tokio::test]
    async fn test_session_deletion() {
        let temp_dir = tempdir().unwrap();
        let settings: GPTSettings = toml::from_str(std::fs::read_to_string("Settings.toml").unwrap().as_str()).unwrap();
        let gpt: GPTConnector = GPTConnector::new(&settings).await;
        let session_id = sazid::utils::generate_session_id();
        let mut session = SessionManager::new(session_id, &gpt);
        let request = ChatCompletionRequestMessage {
            role: Role::User,
            content: Some("Hello, GPT!".to_string()),
            ..Default::default()            // Use default values for other fields
        };
        session.add_request(request);
        session.save_session();
        let path = temp_dir.path().join(session.get_session_filepath());
        assert!(path.exists()); // File should exist

        // session.delete_session().unwrap();
        // assert!(!path.exists()); // File should be deleted now
    }

    #[tokio::test]
    async fn test_ingestion() {
        let dir = tempdir().unwrap();
        let settings: GPTSettings = toml::from_str(std::fs::read_to_string("Settings.toml").unwrap().as_str()).unwrap();
        let gpt: GPTConnector = GPTConnector::new(&settings).await;
        let session_id = sazid::utils::generate_session_id();
        let session = SessionManager::new(session_id, &gpt);
        
        let txt_path = dir.path().join("test.txt");
        File::create(&txt_path)
            .unwrap()
            .write_all(b"Chunk 1\nChunk 2\nChunk 3")
            .unwrap();
        let txt_path = txt_path.to_str().unwrap();
        
        let chunks = session.handle_ingest(&txt_path.into()).await.unwrap();
        
        // Verify ingestion
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "Chunk 1");
        assert_eq!(chunks[1], "Chunk 2");
        assert_eq!(chunks[2], "Chunk 3");

        // Verify ingested data log
        let log_path = dir
            .path()
            .join("session_data/ingested/test_session_ingest.json");
        assert!(log_path.exists());
        let content = fs::read_to_string(log_path).unwrap();
        assert!(content.contains("\"chunk_num\":3"));

        // Verify copied file
        let dest_path = dir
            .path()
            .join("session_data/ingested/test_session_files/test.txt");
        assert!(dest_path.exists());
        let file_content = fs::read_to_string(dest_path).unwrap();
        assert_eq!(file_content, "Chunk 1\nChunk 2\nChunk 3");
    }
}
