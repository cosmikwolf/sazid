use async_openai::types::Role;
use owo_colors::OwoColorize;
use crossterm::{
    event::{self, KeyCode, KeyEvent, KeyModifiers},
    ExecutableCommand,
    style::Print,
    tty::IsTty,
};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use crate::types::SessionManager;
use crate::types::Opts;

pub struct UI {
    stdout: std::io::Stdout,
    user_input: String,
    session_manager: SessionManager,
    opts: Opts
}

impl UI {
    pub fn init(session_manager: SessionManager, opts: Opts) -> Self {
        let stdout = io::stdout();
        let user_input = Self::get_piped_input();
        let mut ui = Self {
            stdout,
            user_input,
            session_manager,
            opts
        };
        ui.setup().unwrap();
        ui
    }

    fn get_piped_input() -> String {
        let mut piped_input = String::new();
        if !io::stdin().is_tty() {
            let _ = io::stdin().read_to_string(&mut piped_input);
        }
        piped_input
    }

    fn setup(&mut self) -> io::Result<()> {
        // execute!(self.stdout, EnterAlternateScreen, MoveTo(0, 0), Hide)?;
        // terminal::enable_raw_mode()?;
        Ok(())
    }

    pub fn teardown(&mut self) -> io::Result<()> {
        // execute!(self.stdout, Show, LeaveAlternateScreen)?;
        // terminal::disable_raw_mode()?;
        Ok(())
    }

    fn display_prompt(&mut self) -> io::Result<()> {
        let prompt = "You: ";
        self.stdout.execute(Print(prompt))?;
        self.stdout.flush()?;
        Ok(())
    }

    fn execute_input(&mut self) -> io::Result<()> {
        let chat_choices = self.session_manager.submit_input(&self.user_input);

        for choice in chat_choices.unwrap() {
            self.display_chat_message(choice.message.role.clone(), choice.message.content.clone().unwrap_or_default());
        }
        self.stdout.flush().unwrap();
        Ok(())
    }

    // Display a message to the user.
    pub fn display_chat_message(&mut self, role: Role, message: String) {
        if self.opts.batch {
            write!(self.stdout, "{}\n\r", message).unwrap();
        } else {
            match role {
                Role::User => write!(self.stdout, "You: {}\n\r", message.blue()),
                Role::Assistant => write!(self.stdout, "GPT: {}\n\r", message.green()),
                _ => Ok(())
            }.unwrap();
        }
        self.stdout.flush().unwrap()
    }

    pub fn display_general_message(&mut self, message: String) {
        write!(self.stdout, "{}\r\n", message).unwrap();
        self.stdout.flush().unwrap();
    }

    pub fn display_debug_message(&mut self, message: String) {
        write!(self.stdout, "Debug: {}\r\n", message.yellow()).unwrap();
        self.stdout.flush().unwrap();
    }
    // Display a error message.
    pub fn display_error_message(&mut self, message: String) {
        write!(self.stdout, "Error: {}\r\n", message.red()).unwrap();
        self.stdout.flush().unwrap();
    }

    // Display a startup message.
    pub fn display_startup_message(&mut self) {
        write!( self.stdout, "Starting interactive GPT chat session. Type 'exit' or 'quit' to end.\n\r",) .unwrap();
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

    pub fn run_interface_loop(&mut self,  batch: bool) -> io::Result<()> {
        let mut exit_flag = false;

        loop {
            
            // check piped input for data
            if !self.user_input.is_empty() {
                self.execute_input().unwrap();
                self.user_input.clear();
                if batch {
                    self.teardown().unwrap();
                    return Ok(())
                }
                continue;
            } else {
                self.display_prompt()?;
            }

            loop {
                if let event::Event::Key(key_event) = event::read()? {
                    match key_event {
                        KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: KeyModifiers::CONTROL,
                            ..
                        } => {
                            exit_flag = true;
                            break;
                        }
                        KeyEvent {
                            code: KeyCode::Backspace,
                            ..
                        } => {
                            if !self.user_input.is_empty() {
                                self.user_input.pop();
                                self.stdout.execute(Print("\u{8} \u{8}"))?; // Handle backspace
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Char(c),
                            ..
                        } => {
                            self.user_input.push(c);
                            write!(self.stdout, "{}", c)?;
                            self.stdout.flush()?;
                        }
                        KeyEvent {
                            code: KeyCode::Enter,
                            ..
                        } => {
                            if self.user_input.trim() == "exit" || self.user_input.trim() == "quit" {
                                exit_flag = true;
                            }
                            if self.user_input.trim().len() > 0 {
                                write!(self.stdout, "\n\r")?;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }

            if exit_flag {
                self.display_exit_message();
                self.teardown().unwrap();
                break;
            }

            self.execute_input().unwrap();
            self.user_input.clear();
        }

        Ok(())
    }
    // Read input from the user.
    // pub fn read_input(&mut self, prompt: &str) -> Result<Option<String>> {
        // self.stdout.write_all(prompt.as_bytes()).unwrap();
        // self.stdout.flush().unwrap();
        // Ok(Some(String::from("test")))
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

}

#[derive(Debug, PartialEq)]
pub enum ImportStatus {
    Success,
    Failure,
    Skipped,
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_ui_display_message() {
        // let mut session_data: Option<Session> = None;
        // let settings: GPTSettings = toml::from_str(std::fs::read_to_string("Settings.toml").unwrap().as_str()).unwrap();
        // let mut session_manager = SessionManager::new(settings, session_data);
        // let mut ui = UI::init(session_manager);
        // // Just a simple test to make sure no panic occurs.
        // // Real UI testing would require more advanced techniques.
        // ui.display_chat_message(Role::User, "Test".to_string());
        // ui.display_chat_message(Role::Assistant, "Test".to_string());

        // let sample_path = PathBuf::from("/path/to/file.txt");
        // ui.display_import_message(&sample_path, ImportStatus::Success);
        // ui.display_import_message(&sample_path, ImportStatus::Failure);
        // ui.display_import_message(&sample_path, ImportStatus::Skipped);
        // ui.teardown().unwrap();
    }
}
