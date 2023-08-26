use owo_colors::OwoColorize;

pub fn display_welcome_message() {
    println!("{}", "Welcome to Voracious!".green().bold());
    println!("Type 'help' for a list of commands.");
}

pub fn display_error_message(error: &str) {
    println!("{}", error.red());
}

pub fn display_info_message(info: &str) {
    println!("{}", info.blue());
}

pub fn display_success_message(success: &str) {
    println!("{}", success.green());
}

pub fn prompt() -> String {
    print!("{}", "voracious> ".cyan().bold());
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).expect("Failed to read line");
    input.trim().to_string()
}

pub fn display_help() {
    println!("{}", "Available Commands:".underline());
    println!("help - Display this help message");
    println!("new - Create a new project");
    println!("load - Load an existing project");
    println!("list - List all projects");
    println!("quit - Exit the program");
    // ... add more commands as needed
}

// ... additional CLI functions as needed
