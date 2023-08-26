extern crate clap;
use clap::{App, Arg, SubCommand};
use std::io::{self, Write};

mod project_manager;
mod file_chunker;
mod gpt_connector;

fn main() {
    let matches = App::new("Voracious")
        .version("1.0")
        .author("Your Name <your.email@example.com>")
        .about("Interacts with GPT and manages projects.")
        .subcommand(SubCommand::with_name("new")
            .about("Creates a new project")
            .arg(Arg::with_name("name")
                .help("Name of the new project")
                .required(true)
                .index(1)))
        .subcommand(SubCommand::with_name("load")
            .about("Loads an existing project")
            .arg(Arg::with_name("name")
                .help("Name of the project to load")
                .required(true)
                .index(1)))
        .subcommand(SubCommand::with_name("list")
            .about("Lists all existing projects"))
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("new") {
        let project_name = matches.value_of("name").unwrap();
        project_manager::create_new_project(project_name);
    } else if let Some(matches) = matches.subcommand_matches("load") {
        let project_name = matches.value_of("name").unwrap();
        project_manager::load_project(project_name);
    } else if matches.subcommand_matches("list").is_some() {
        project_manager::list_projects();
    } else {
        // Start a chat session if no arguments are provided
        start_chat_session();
    }
}

fn start_chat_session() {
    let mut session_log = Vec::new();
    loop {
        print!("You: ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        session_log.push(format!("You: {}", input.trim()));

        if input.trim() == "exit" {
            break;
        }

        let response = gpt_connector::chat_with_gpt(&input);
        println!("GPT: {}", response);
        session_log.push(format!("GPT: {}", response));
    }
    project_manager::save_chat_log(session_log);
}
