extern crate sazid;
extern crate tempfile;

#[cfg(test)]
mod integration_tests {
    
    use super::*;
    use std::io::Write;
    use std::fs::{self, File};
    use async_openai::types::Role;
    use sazid::gpt_connector::ChatCompletionRequestMessage;
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
    #[test]
    fn test_session_creation() {
        let session_manager = SessionManager::new(tempdir().unwrap().path().to_path_buf());
        let filename = session_manager.new_session_filename();
        assert!(filename.contains("_")); // Check if filename contains the expected delimiter
    }

    // 2. Test generation of multiple unique session filenames
    // Requirement: Multiple sessions should have unique identifiers.
    #[test]
    fn test_multiple_sessions() {
        let session_manager = SessionManager::new(tempdir().unwrap().path().to_path_buf());
        let filename1 = session_manager.new_session_filename();
        let filename2 = session_manager.new_session_filename();
        assert_ne!(filename1, filename2);
    }

    // 3. Test storage of messages within a session
    // Requirement: The application should be able to store messages (text) in a session.
    #[test]
    fn test_message_storage() {
        let mut messages = vec![];
        let user_message = ChatCompletionRequestMessage {
            role: Role::User,
            content: "Hello, GPT!".to_string(),
        };
        messages.push(user_message);
        assert_eq!(messages[0].content, "Hello, GPT!");
    }

    // 4. Test saving and loading of sessions, as well as tracking the last session
    // Requirement: The application should save and reload chat sessions. It should also track the most recent session for easy reloading.
    #[test]
    fn test_session_save_and_load() {
        let temp_dir = tempdir().unwrap();
        let session_manager = SessionManager::new(temp_dir.path().to_path_buf());
        let messages = vec![ChatCompletionRequestMessage {
            role: Role::User,
            content: "Hello, GPT!".to_string(),
        }];
        let filename = session_manager.new_session_filename();

        // Save session and last session
        session_manager.save_session(&filename, &messages).unwrap();
        session_manager.save_last_session_filename(&filename).unwrap();

        // Check if file exists
        assert!(temp_dir
            .path()
            .join("session_data")
            .join(&filename)
            .exists());

        // Load specific session
        let loaded_messages = session_manager.load_session(&filename).unwrap();
        assert_eq!(loaded_messages[0].content, "Hello, GPT!");

        // Load last session
        let last_session_filename = session_manager.load_last_session_filename().unwrap();
        let last_session = session_manager.load_session(&last_session_filename).unwrap();
        assert_eq!(last_session[0].content, "Hello, GPT!");
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
    #[test]
    fn test_continued_conversation() {
        let temp_dir = tempdir().unwrap();
        let session_manager = SessionManager::new(temp_dir.path().to_path_buf());
        let mut messages = vec![
            ChatCompletionRequestMessage {
                role: Role::User,
                content: "Hello, GPT!".to_string(),
            },
            ChatCompletionRequestMessage {
                role: Role::Assistant,
                content: "Hello, User!".to_string(),
            },
        ];

        let filename = session_manager.new_session_filename();
        session_manager.save_session(&filename, &messages).unwrap();
        session_manager.save_last_session_filename(&filename).unwrap();

        let loaded_messages = session_manager.load_session(&filename).unwrap();
        assert_eq!(loaded_messages.len(), 2); // Two messages in the session

        let new_message = ChatCompletionRequestMessage {
            role: Role::User,
            content: "How are you?".to_string(),
        };
        messages.push(new_message);
        assert_eq!(messages.len(), 3); // Now, three messages in the session
    }

    // 9. Test the ability to delete a session
    // Requirement: The application should provide functionality to delete a chat session.
    #[test]
    fn test_session_deletion() {
        let temp_dir = tempdir().unwrap();
        let session_manager = SessionManager::new(temp_dir.path().to_path_buf());
        let messages = vec![ChatCompletionRequestMessage {
            role: Role::User,
            content: "Hello, GPT!".to_string(),
        }];

        let filename = session_manager.new_session_filename();
        session_manager.save_session(&filename, &messages).unwrap();
        let path = temp_dir.path().join("session_data").join(&filename);
        assert!(path.exists()); // File should exist

        session_manager.delete_session(&filename).unwrap();
        assert!(!path.exists()); // File should be deleted now
    }

    #[test]
    fn test_ingestion() {
        let dir = tempdir().unwrap();
        let manager = SessionManager::new(dir.path().to_path_buf());

        let txt_path = dir.path().join("test.txt");
        File::create(&txt_path)
            .unwrap()
            .write_all(b"Chunk 1\nChunk 2\nChunk 3")
            .unwrap();

        let chunks = manager.handle_ingest(&txt_path).unwrap();

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
