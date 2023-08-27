use std::path::PathBuf;
use std::io;
use owo_colors::OwoColorize;

mod project_manager;
mod gpt_connector;
mod file_chunker;
mod logger;
mod utils;

use project_manager::Project;
use gpt_connector::GPTConnector;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() == 1 {
        // No arguments provided, start interactive mode
        start_interactive_mode()?;
    } else {
        // Handle CLI arguments
        match args[1].as_str() {
            "--list-projects" => {
                // List all projects
                // TODO: Implement listing of all projects
            }
            "--project-details" => {
                if args.len() < 3 {
                    println!("{}", "Please provide a project name.".red());
                    return Ok(());
                }
                // Print details of a specific project
                let _project_name = &args[2];
                // TODO: Implement printing of project details
            }
            "--delete-project" => {
                if args.len() < 3 {
                    println!("{}", "Please provide a project name.".red());
                    return Ok(());
                }
                // Delete a specific project
                let _project_name = &args[2];
                // TODO: Implement deletion of a project
            }
            _ => {
                println!("{}", "Invalid command. Use --help for available commands.".red());
            }
        }
    }

    Ok(())
}

fn start_interactive_mode() -> io::Result<()> {
    let gpt = GPTConnector::new();

    // Check if there's a previous project
    let project_path = PathBuf::from("path_to_last_project"); // This should be loaded from some config or state
    let project = Project::load_project(project_path);

    if let Some(proj) = project {
        println!("Loaded project: {}", proj.get_path().display().green());
    } else {
        println!("No previous project found. Starting a new session.");
    }

    loop {
        let mut input = String::new();
        print!("You: ");
        use std::io::Write;
io::stdout().flush()?;
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input == "exit" {
            break;
        }

        let response = tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(gpt.send_request("gpt-3.5-turbo", input)).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?.content;
        println!("GPT: {}", response.green());
    }

    Ok(())
}
