use async_openai::types::Role;
use rustyline::error::ReadlineError;
use owo_colors::OwoColorize;
use std::path::PathBuf;

pub struct UI;

impl UI {
    // Read input from the user.
    pub fn read_input(prompt: &str) -> Result<String, ReadlineError> {
        let mut rl = rustyline::DefaultEditor::new()?;
        rl.readline(prompt)
    }

    // Display a message to the user.
    pub fn display_message(role: Role, message: &str) {
        match role {
            Role::User => println!("You: {}", message),
            Role::Assistant => println!("GPT: {}", message.green()),
            _ => {}
        }
    }

    // Display a startup message.
    pub fn display_startup_message() {
        println!("Starting interactive GPT chat session. Type 'exit' or 'quit' to end.");
    }

    // Display an exit message.
    pub fn display_exit_message() {
        println!("Exiting gracefully. Goodbye!");
    }

    // Display a message about the import process.
    pub fn display_import_message(file: &PathBuf, status: ImportStatus) {
        match status {
            ImportStatus::Success => println!("Successfully imported: {}", file.display().blue()),
            ImportStatus::Failure => println!("Failed to import: {}", file.display().red()),
            ImportStatus::Skipped => println!("Skipped importing: {}", file.display().yellow()),
        }
    }

    // Display a message about starting the import process.
    pub fn display_import_start_message() {
        println!("Starting import process. Press Ctrl-C to skip a file. Press Ctrl-C twice quickly to cancel.");
    }

    // Display a message about the conclusion of the import process.
    pub fn display_import_end_message() {
        println!("Import process completed.");
    }
}

#[derive(Debug, PartialEq)]
pub enum ImportStatus {
    Success,
    Failure,
    Skipped,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_display_message() {
        // Just a simple test to make sure no panic occurs.
        // Real UI testing would require more advanced techniques.
        UI::display_message(Role::User, "Test");
        UI::display_message(Role::Assistant, "Test");

        let sample_path = PathBuf::from("/path/to/file.txt");
        UI::display_import_message(&sample_path, ImportStatus::Success);
        UI::display_import_message(&sample_path, ImportStatus::Failure);
        UI::display_import_message(&sample_path, ImportStatus::Skipped);
    }
}
