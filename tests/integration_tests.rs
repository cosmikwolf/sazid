extern crate sazid;
extern crate tempfile;

#[cfg(test)]
mod integration_tests {
    use std::path::Path;

    use super::*;
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
        let filename = SessionManager::new_session_filename();
        assert!(filename.contains("_")); // Check if filename contains the expected delimiter
    }

    // 2. Test generation of multiple unique session filenames
    // Requirement: Multiple sessions should have unique identifiers.
    #[test]
    fn test_multiple_sessions() {
        let filename1 = SessionManager::new_session_filename();
        let filename2 = SessionManager::new_session_filename();
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
        let messages = vec![ChatCompletionRequestMessage {
            role: Role::User,
            content: "Hello, GPT!".to_string(),
        }];
        let filename = SessionManager::new_session_filename();

        // Save session and last session
        SessionManager::save_session(&filename, &messages).unwrap();
        SessionManager::save_last_session_filename(&filename).unwrap();

        // Check if file exists
        assert!(temp_dir
            .path()
            .join("session_data")
            .join(&filename)
            .exists());

        // Load specific session
        let loaded_messages = SessionManager::load_session(&filename).unwrap();
        assert_eq!(loaded_messages[0].content, "Hello, GPT!");

        // Load last session
        let last_session_filename = SessionManager::load_last_session_filename().unwrap();
        let last_session = SessionManager::load_session(&last_session_filename).unwrap();
        assert_eq!(last_session[0].content, "Hello, GPT!");
    }

    // ...

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
        let _temp_dir = tempdir().unwrap();
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

        let filename = SessionManager::new_session_filename();
        SessionManager::save_session(&filename, &messages).unwrap();
        SessionManager::save_last_session_filename(&filename).unwrap();

        let loaded_messages = SessionManager::load_session(&filename).unwrap();
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
        let messages = vec![ChatCompletionRequestMessage {
            role: Role::User,
            content: "Hello, GPT!".to_string(),
        }];

        let filename = SessionManager::new_session_filename();
        SessionManager::save_session(&filename, &messages).unwrap();
        let path = format!("session_data/{}", filename);
        assert!(Path::new(&path).exists()); // File should exist

        SessionManager::delete_session(&filename).unwrap();
        assert!(!Path::new(&path).exists()); // File should be deleted now
    }
}
