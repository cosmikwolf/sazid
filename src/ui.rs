use async_openai::types::Role;
use clap::Parser;
use owo_colors::OwoColorize;
use std::io::{stdin, stdout, Error, Stdin, Stdout, Write};
use termion::input::TermRead;
use termion::raw::IntoRawMode;

use std::{ffi::OsString, path::PathBuf};

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
pub struct UI {
    pub stdin: std::io::StdinLock<'static>,
    pub stdout: termion::raw::RawTerminal<std::io::StdoutLock<'static>>,
}

impl UI {
    // Initialize the UI.
    pub fn init() -> Self {
        // initialize termion
        let stdout = stdout();
        let mut stdout = stdout.lock().into_raw_mode().unwrap();
        let stdin = stdin();
        let stdin = stdin.lock();
        let mut ui = Self { stdin, stdout };
        // Display a startup message.
        ui.display_startup_message();
        return ui;
    }
    // Read input from the user.
    pub fn read_input(&mut self, prompt: &str) -> Result<Option<String>, Error> {
        self.stdout.write_all(prompt.as_bytes()).unwrap();
        self.stdout.flush().unwrap();
        self.stdin.read_line()
    }
    pub fn read_stdin(&mut self, message: String) {
        write!(self.stdout, "stdin: {}\n", message.magenta());
    }
    // Display a message to the user.
    pub fn display_chat_message(&mut self, role: Role, message: String) {
        match role {
            Role::User => write!(self.stdout, "You: {}\n", message.blue()),
            Role::Assistant => write!(self.stdout, "GPT: {}\n", message.green()),
            _ => Ok(())
        };
    }

    pub fn display_general_message(&mut self, message: String) {
        write!(self.stdout, "{}", message);
    }

    pub fn display_debug_message(&mut self, message: String) {
        write!(self.stdout, "Debug: {}", message.yellow());
    }
    // Display a error message.
    pub fn display_error_message(&mut self, message: String) {
        write!(self.stdout, "Error: {}", message.red());
    }

    // Display a startup message.
    pub fn display_startup_message(&mut self) {
        write!(
            self.stdout,
            "{}{}{}yStarting interactive GPT chat session. Type 'exit' or 'quit' to end.{}{}\n",
            termion::clear::All,
            termion::cursor::Goto(5, 5),
            termion::style::Bold,
            termion::style::Reset,
            termion::cursor::Goto(20, 10)
        )
        .unwrap();
        self.stdout.flush().unwrap();
    }

    // Display an exit message.
    pub fn display_exit_message(&mut self) {
        write!(self.stdout, "Exiting gracefully. Goodbye!");
    }

    // Display each interaction in the chat history
    pub fn display_chat_history(&mut self, chat_history: &Vec<(Role, String)>) {
        for (role, message) in chat_history {
            self.display_chat_message(role.clone(), message.clone());
        }
    }

    // Display a message about the import process.
    pub fn display_import_message(&mut self, file: &PathBuf, status: ImportStatus) {
        match status {
            ImportStatus::Success => write!(self.stdout, "Successfully imported: {}", file.display().blue()).unwrap(),
            ImportStatus::Failure => write!(self.stdout, "Failed to import: {}", file.display().red()).unwrap(),
            ImportStatus::Skipped => write!(self.stdout, "Skipped importing: {}", file.display().yellow()).unwrap(),
        }
    }

    // Display a message about starting the import process.
    pub fn display_import_start_message(&mut self) {
        write!(self.stdout, "Starting import process. Press Ctrl-C to skip a file. Press Ctrl-C twice quickly to cancel.");
    }

    // Display a message about the conclusion of the import process.
    pub fn display_import_end_message(&mut self) {
        write!(self.stdout, "Import process completed.");
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
        let ui = UI::init();
        // Just a simple test to make sure no panic occurs.
        // Real UI testing would require more advanced techniques.
        ui.display_chat_message(Role::User, "Test".to_string());
        ui.display_chat_message(Role::Assistant, "Test".to_string());

        let sample_path = PathBuf::from("/path/to/file.txt");
        ui.display_import_message(&sample_path, ImportStatus::Success);
        ui.display_import_message(&sample_path, ImportStatus::Failure);
        ui.display_import_message(&sample_path, ImportStatus::Skipped);
    }
}
