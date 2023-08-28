mod gpt_connector;
mod logger;

use owo_colors::OwoColorize;
use gpt_connector::{GPTConnector, ChatCompletionRequestMessage};
use logger::Logger;
use rustyline::error::ReadlineError;
use async_openai::types::Role;
use std::fs;
use clap::Parser;

#[derive(Parser)]
#[clap(version = "0.1.0", author = "Author")]
struct Opts {
    /// Start a new chat session
    #[clap(long)]
    new_session: bool,

    /// Continue an arbitrary session by providing its name
    #[clap(long)]
    session: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();

    let gpt = GPTConnector::new();
    let logger = Logger::new();

    println!("Starting interactive GPT chat session. Type 'exit' to end.");
    
    let mut rl = rustyline::DefaultEditor::new()?;
    if rl.load_history("logs/history.txt").is_err() {
        println!("No previous history found.");
    }

    // Load the previous session if it exists and no flags are provided
    let mut messages: Vec<ChatCompletionRequestMessage> = if !opts.new_session && opts.session.is_none() && fs::read("logs/session.bincode").is_ok() {
        let data = fs::read("logs/session.bincode")?;
        bincode::deserialize(&data).unwrap_or_default()
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

                if input == "exit" {
                    // Save the current session
                    let data = bincode::serialize(&messages)?;
                    fs::write("logs/session.bincode", data)?;
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
                    .block_on(gpt.send_request(messages.clone()))?; // pass a clone to retain the original

                let assistant_message = ChatCompletionRequestMessage {
                    role: response.role,
                    content: response.content.clone(),
                };
                messages.push(assistant_message);

                logger.log_interaction(&user_message.content, &response.content);

                let _ = rl.add_history_entry(&user_message.content);
                println!("GPT: {}", response.content.green());
            },
            Err(ReadlineError::Interrupted) => {
                println!("Interrupted");
                break;
            },
            Err(ReadlineError::Eof) => {
                println!("EOF reached");
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
