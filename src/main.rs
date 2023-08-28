mod gpt_connector;
mod logger;

use clap::Parser;
use gpt_connector::{ChatCompletionRequestMessage, GPTConnector};
use logger::Logger;
use rustyline::error::ReadlineError;
use async_openai::types::Role;
use std::fs;
use serde_json;
use owo_colors::OwoColorize;
use chrono::Local;

#[derive(Parser)]
#[clap(
    version = "1.0",
    author = "Your Name",
    about = "Interactive chat with GPT"
)]
struct Opts {
    #[clap(short = 'n', long, help = "Start a new chat session")]
    new: bool,
    
    #[clap(short = 'c', long, help = "Continue from a specified session file")]
    continue_session: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();

    let gpt = GPTConnector::new();
    let logger = Logger::new();

    println!("Starting interactive GPT chat session. Type 'exit' or 'quit' to end, or use Ctrl-C.");
    
    let mut rl = rustyline::DefaultEditor::new()?;
    if rl.load_history("logs/history.txt").is_err() {
        println!("No previous history found.");
    }

    let session_filename = match opts.continue_session {
        Some(filename) => filename,
        None => fs::read_to_string("logs/last_session.txt").unwrap_or_else(|_| format!("logs/session_{}.json", Local::now().format("%Y-%m-%d_%H-%M")))
    };

    let mut messages: Vec<ChatCompletionRequestMessage> = if !opts.new {
        let data = fs::read(&session_filename).unwrap_or_default();
        serde_json::from_slice(&data).unwrap_or_default()
    } else {
        vec![]
    };

    for message in &messages {
        match message.role {
            Role::User => println!("You (from previous session): {}", message.content),
            Role::Assistant => println!("GPT (from previous session): {}", message.content.green()),
            _ => {}
        }
    }

    loop {
        let readline = rl.readline("You: ");
        match readline {
            Ok(line) => {
                let input = line.trim();

                if input == "exit" || input == "quit" {
                    println!("Exiting gracefully. Saving session...");
                    let data = serde_json::to_vec(&messages)?;
                    fs::write(&session_filename, data)?;
                    fs::write("logs/last_session.txt", session_filename)?;
                    break;
                }

                let user_message = ChatCompletionRequestMessage {
                    role: Role::User,
                    content: input.to_string(),
                };
                messages.push(user_message.clone());

                let response = tokio::runtime::Builder::new_current_thread()
                    .enable_io()
                    .enable_time()
                    .build()?
                    .block_on(gpt.send_request(messages.clone()))?;

                let assistant_message = ChatCompletionRequestMessage {
                    role: response.role,
                    content: response.content.clone(),
                };
                messages.push(assistant_message);

                logger.log_interaction(&user_message.content, &response.content);

                let _ = rl.add_history_entry(&user_message.content);
                println!("GPT: {}", response.content.green());
            },
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                println!("Exiting gracefully. Saving session...");
                let data = serde_json::to_vec(&messages)?;
                fs::write(&session_filename, data)?;
                fs::write("logs/last_session.txt", session_filename)?;
                break;
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    rl.save_history("logs/history.txt")?;

    Ok(())
}
