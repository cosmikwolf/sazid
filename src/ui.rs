use async_openai::types::Role;
use crossterm::{
    cursor::{Hide, Show},
    event::{read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, window_size
    },
};
use owo_colors::OwoColorize;
use std::{io::{stdout, StdoutLock, Result, Write}, path::PathBuf};
use tui_input::backend::crossterm as backend;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;


pub struct UI {
    input: Input,
    stdout: StdoutLock<'static>
}

impl UI {
        // Initialize the UI.
        pub fn init() -> Self {
            // initialize tui
            enable_raw_mode().unwrap();
            let stdout = stdout();
            let stdout = stdout.lock();
            let input: Input = "Hello ".into();
            // Display a startup message.
            let mut ui = Self {
                input,
                stdout
            };
            execute!(ui.stdout, Hide, EnterAlternateScreen, EnableMouseCapture).unwrap();
            let window_size = window_size().unwrap();
            
            backend::write(&mut ui.stdout, ui.input.value(), ui.input.cursor(), (0, 0), window_size.width ).unwrap();
            ui.stdout.flush().unwrap();
    
            // ui.display_startup_message();
            return ui;
        }
    pub fn cleanup_interface(&mut self) -> Result<()> {
        execute!(self.stdout, Show, LeaveAlternateScreen, DisableMouseCapture)?;
        disable_raw_mode()?;
        println!("{}", self.input);
        Ok(())
    }
    pub fn interface_loop(&mut self) -> Result<()> {
        loop {
            let event = read()?;
    
            if let Event::Key(KeyEvent { code, .. }) = event {
                match code {
                    KeyCode::Esc | KeyCode::Enter => {
                        break;
                    }
                    _ => {
                        if self.input.handle_event(&event).is_some() {
                            backend::write(
                                &mut self.stdout,
                                self.input.value(),
                                self.input.cursor(),
                                (0, 0),
                                15,
                            )?;
                            self.stdout.flush().unwrap();
                        }
                    }
                }
            }
        }
        self.cleanup_interface().unwrap();
        Ok(())
    }
    // Read input from the user.
    pub fn read_input(&mut self, prompt: &str) -> Result<Option<String>> {
        // self.stdout.write_all(prompt.as_bytes()).unwrap();
        // self.stdout.flush().unwrap();
        Ok(Some(String::from("test")))
        // let mut input_buf: Vec<u8> = Vec::new();
        // self.stdin.read_line()
        // Ok(Some(input_buf))
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
