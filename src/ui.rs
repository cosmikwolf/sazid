use async_openai::types::Role;
use owo_colors::OwoColorize;
use std::{io::{stdin, stdout, Error, Write, Read}};
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use std::path::PathBuf;

pub struct UI {
    pub stdin: std::io::StdinLock<'static>,
    pub stdout: termion::raw::RawTerminal<std::io::StdoutLock<'static>>,
    // pub bytes:  std::io::Bytes<std::io::StdinLock<'static>>,
}

impl UI {
    // Initialize the UI.
    pub fn init() -> Self {
        // initialize termion
        let stdout = stdout();
        let stdout = stdout.lock().into_raw_mode().unwrap();
        let stdin = stdin();
        let stdin = stdin.lock();
        // let bytes = stdin.bytes();
        let mut ui = Self { 
            stdin, 
            stdout, 
            // bytes
        };
        // Display a startup message.
        ui.display_startup_message();
        return ui;
    }
    // Read input from the user.
    pub fn read_input(&mut self, prompt: &str) -> Result<Option<String>, Error> {
        self.stdout.write_all(prompt.as_bytes()).unwrap();
        self.stdout.flush().unwrap();

        // let mut input_buf: Vec<u8> = Vec::new();
        let input_buf = self.stdin.read_line().unwrap().unwrap();
        Ok(Some(input_buf))
        /*
        loop {
            // let b = self.bytes.next().unwrap().unwrap();
            // write!(self.stdout, "{}", b as char).unwrap();
            self.stdout.write_all(&[b]).unwrap();
            input_buf.push(b);
            self.stdout.flush().unwrap();
            match b {
                0x7f => {
                    &input_buf.pop();
                }
                b'\r' => {
                    let input =  String::from_utf8(input_buf).unwrap();
                    match input.as_str() {
                        "exit\r" | "quit\r" => {
                            self.display_exit_message();
                            return Ok(None);
                        }
                        "clear\r" => {
                            write!(self.stdout, "{}", termion::clear::All ).unwrap();
                            self.stdout.flush().unwrap();
                            input_buf.clear();
                        }
                        _ => {
                            self.stdout.write_all(b"\n\r").unwrap();
                            self.stdout.flush().unwrap();
                            return Ok(Some(input));
                        }
                    }
                }
                _ => {
                    // self.stdout.write_all( "{}",b).unwrap();
                    // self.stdout.flush().unwrap();
                }
            }
        }
        */
    }

    // Display a message to the user.
    pub fn display_chat_message(&mut self, role: Role, message: String) {
        match role {
            Role::User => writeln!(self.stdout, "You: {}\n", message.blue()),
            Role::Assistant => writeln!(self.stdout, "GPT: {}\n", message.green()),
            _ => Ok(())
        }.unwrap()
    }

    pub fn display_general_message(&mut self, message: String) {
        write!(self.stdout, "{}", message).unwrap();
    }

    pub fn display_debug_message(&mut self, message: String) {
        writeln!(self.stdout, "Debug: {}", message.yellow()).unwrap();
    }
    // Display a error message.
    pub fn display_error_message(&mut self, message: String) {
        write!(self.stdout, "Error: {}", message.red()).unwrap();
    }

    // Display a startup message.
    pub fn display_startup_message(&mut self) {
        write!(
            self.stdout,
            "{}{}{}{}Starting interactive GPT chat session. Type 'exit' or 'quit' to end.\n\r",
            termion::clear::All,
            termion::cursor::Goto(1,1),
            termion::style::Bold,
            termion::style::Reset,
        )
        .unwrap();
        self.stdout.flush().unwrap();
    }

    // Display an exit message.
    pub fn display_exit_message(&mut self) {
        write!(self.stdout, "Exiting gracefully. Goodbye!{}", termion::style::Reset).unwrap();
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
        write!(self.stdout, "Starting import process. Press Ctrl-C to skip a file. Press Ctrl-C twice quickly to cancel.").unwrap();
    }

    // Display a message about the conclusion of the import process.
    pub fn display_import_end_message(&mut self) {
        write!(self.stdout, "Import process completed.").unwrap();
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
        let mut ui = UI::init();
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
