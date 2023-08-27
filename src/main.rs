mod gpt_connector;
mod logger;

use owo_colors::OwoColorize;
use std::io;
use gpt_connector::GPTConnector;
use logger::Logger;

fn main() -> io::Result<()> {
    let gpt = GPTConnector::new();
    let logger = Logger::new();

    println!("Starting interactive GPT chat session. Type 'exit' to end.");
    
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

        let response = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap()
            .block_on(gpt.send_request(input))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
            .content;
        
        logger.log_interaction(input, &response);

        println!("GPT: {}", response.green());
    }

    Ok(())
}

