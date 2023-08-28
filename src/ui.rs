use async_openai::types::Role;
use rustyline::error::ReadlineError;
use owo_colors::OwoColorize;

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
    }
}
