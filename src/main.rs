mod gpt_connector;
mod logger;

use owo_colors::OwoColorize;
use gpt_connector::GPTConnector;
use logger::Logger;
use rustyline::error::ReadlineError;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpt = GPTConnector::new();
    let logger = Logger::new();

    println!("Starting interactive GPT chat session. Type 'exit' to end.");
    
    let mut rl = rustyline::DefaultEditor::new()?;
    if rl.load_history("logs/history.txt").is_err() {
        println!("No previous history found.");
    }
    
    loop {
        let readline = rl.readline("You: ");
        match readline {
            Ok(line) => {
                let input = line.trim();

                if input == "exit" {
                    break;
                }

                let response = tokio::runtime::Builder::new_current_thread()
                    .enable_io()
                    .enable_time()
                    .build()?
                    .block_on(gpt.send_request(input))
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
                    .content;
                
                logger.log_interaction(input, &response);
                
                let _ = rl.add_history_entry(input);
                println!("GPT: {}", response.green());
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
