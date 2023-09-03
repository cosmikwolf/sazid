use async_openai::types::Role;
use clap::Parser;
use owo_colors::OwoColorize;
use rustyline::error::ReadlineError;
use std::{ffi::OsString, path::PathBuf};
pub struct UI;

#[derive(Parser)]
#[clap(
    version = "1.0",
    author = "Your Name",
    about = "Interactive chat with GPT"
)]
pub struct Opts {
    #[clap(
        short = 'm',
        long,
        value_name = "MODEL_NAME",
        help = "Specify the model to use (e.g., gpt-4, gpt-3.5-turbo-16k)"
    )]
    pub model: Option<String>,

    #[clap(
        short = 'l',
        long = "list-models",
        help = "List the models the user has access to"
    )]
    pub list_models: bool,

    #[clap(short = 'n', long, help = "Start a new chat session")]
    pub new: bool,

    #[clap(short = 'c', long, help = "Continue from a specified session file")]
    pub continue_session: Option<String>,

    #[clap(
        short = 'i',
        long,
        value_name = "PATH",
        help = "Import a file or directory for GPT to process"
    )]
    pub ingest: Option<OsString>,

    // // write a positional argument that will be loaded into a string
    // #[clap(
    //     value_name = "text",
    //     help = "you can pipe data into sazid in order to ingest from stdin"
    // )]
    // pub stdin: Option<OsString>,
}

impl UI {
    // Read input from the user.
    pub fn read_input(prompt: &str) -> Result<String, ReadlineError> {
        println!("readinput");
        let mut rl = rustyline::DefaultEditor::new()?;
        rl.readline(prompt)
    }
    pub fn read_stdin(message: String) {
        println!("stdin: {}", message.magenta())
    }
    // Display a message to the user.
    pub fn display_message(role: Role, message: String) {
        match role {
            Role::User => println!("You: {}", message.blue()),
            Role::Assistant => println!("GPT: {}", message.green()),
            _ => {}
        }
    }

    pub fn display_debug_message(message: String) {
        println!("Debug: {}", message.yellow());
    }
    // Display a error message.
    pub fn display_error_message(message: String) {
        println!("Error: {}", message.red());
    }

    // Display a startup message.
    pub fn display_startup_message() {
        println!("Starting interactive GPT chat session. Type 'exit' or 'quit' to end.");
    }

    // Display an exit message.
    pub fn display_exit_message() {
        println!("Exiting gracefully. Goodbye!");
    }

    // Display each interaction in the chat history
    pub fn display_chat_history(chat_history: &Vec<(Role, String)>) {
        for (role, message) in chat_history {
            Self::display_message(role.clone(), message.clone());
        }
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
        UI::display_message(Role::User, "Test".to_string());
        UI::display_message(Role::Assistant, "Test".to_string());

        let sample_path = PathBuf::from("/path/to/file.txt");
        UI::display_import_message(&sample_path, ImportStatus::Success);
        UI::display_import_message(&sample_path, ImportStatus::Failure);
        UI::display_import_message(&sample_path, ImportStatus::Skipped);
    }
}
