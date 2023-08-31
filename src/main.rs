
use config::{Config, File};
use serde::Deserialize;
use async_openai::types::ChatCompletionRequestMessage;
use async_openai::types::Role;
use clap::Parser;
use sazid::gpt_connector::GPTConnector;
use sazid::session_manager::SessionManager;
use sazid::ui::UI;
use std::ffi::OsString;
use std::path::PathBuf;
use rustyline::error::ReadlineError;

#[derive(Parser)]
#[clap(
    version = "1.0",
    author = "Your Name",
    about = "Interactive chat with GPT"
)]
struct Opts {
    #[clap(
        short = 'm',
        long,
        value_name = "MODEL_NAME",
        help = "Specify the model to use (e.g., gpt-4, gpt-3.5-turbo-16k)"
    )]
    model: Option<String>,

    #[clap(
        short = 'l',
        long = "list-models",
        help = "List the models the user has access to"
    )]
    list_models: bool,

    #[clap(short = 'n', long, help = "Start a new chat session")]
    new: bool,

    #[clap(short = 'c', long, help = "Continue from a specified session file")]
    continue_session: Option<String>,

    #[clap(
        short = 'i',
        long,
        value_name = "PATH",
        help = "Import a file or directory for GPT to process"
    )]
    ingest: Option<OsString>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();

    let gpt = GPTConnector::new();
    
    let config = load_config()?;

    let settings = Config::builder()
        // Add in `./Settings.toml`
        .add_source(config::File::with_name("Settings"))
        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
        .build()
        .unwrap();
    
    // Handle model selection based on CLI flag
    if let Some(model_name) = &opts.model {
        // In a real-world scenario, you would set the selected model in the session manager or GPT connector
        println!("Using model: {}", model_name);
    }

    // Handle listing models based on CLI flag
    if opts.list_models {
        // In a real-world scenario, you would call the OpenAI API to list models the user has access to
        println!("Listing accessible models...");
        println!("gpt-3.5-turbo-16k");
        println!("gpt-4");
        println!("gpt-4-32k");
        return Ok(());
    }

    if let Some(path) = &opts.ingest {
        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()?
            .block_on(session_manager.handle_ingest(&path.to_string_lossy().to_string()))?;
    }

    UI::display_startup_message();

    let mut messages: Vec<ChatCompletionRequestMessage> = if !opts.new {
        match opts.continue_session {
            Some(session_file) => session_manager.load_session(&session_file)?,
            None => {
                if let Some(last_session) = session_manager.load_last_session_filename() {
                    session_manager.load_session(&last_session)?
                } else {
                    vec![]
                }
            }
        }
    } else {
        vec![]
    };

    for message in &messages {
        UI::display_message(message.role.clone(), &message.content.unwrap_or_default());
    }

    loop {
        match UI::read_input("You: ") {
            Ok(input) => {
                let input = input.trim();

                if input.starts_with("ingest ") {
                    let filepath = input.split_whitespace().nth(1).unwrap_or_default();
                    tokio::runtime::Builder::new_current_thread()
                        .enable_io()
                        .enable_time()
                        .build()?
                        .block_on(session_manager.handle_ingest(&filepath.to_string()))?;
                } else {
                    if input == "exit" || input == "quit" {
                        session_manager.save_last_session_filename(session_manager.session_filename)?;
                        UI::display_exit_message();
                        break;
                    }
                    let user_message = ChatCompletionRequestMessage {
                        role: Role::User,
                        content: Some(input.to_string()),
                        function_call: None, // If you have appropriate data, replace None
                        name: None,          // If you have appropriate data, replace None
                    };
                    messages.push(user_message.clone());

                    match tokio::runtime::Builder::new_current_thread()
                        .enable_io()
                        .enable_time()
                        .build()?
                        .block_on(gpt.send_request(vec![input.to_string()]))
                    {
                        Ok(response) => {
                            for choice in &response.choices {
                                UI::display_message(
                                    choice.message.role,
                                    &choice.message.content.unwrap_or_default(),
                                );
                            }
                            session_manager.save_chat_to_session(
                                &session_filename,
                                &messages,
                                &response,
                            )?;
    
                        }
                        Err(error) => {
                            // Displaying the error to the user
                            UI::display_message(Role::System, &format!("Error: {}", error));

                            // Logging the request and the error
                            // NOTE: We'll need an instance or reference to the session manager here to call save_chat_to_session
                            // session_manager.save_chat_to_session("error_log.json", &vec![input.to_string()], &None).expect("Failed to save error log");
                        }
                    }
                }
            }
            Err(e) => {
                println!("Error sending request to GPT: {:?}", e);
            }
            Err(ReadlineError::Interrupted) => {
                let session_filename = session_manager.new_session_filename();
                session_manager.save_chat_to_session(&session_filename, &messages)?;
                session_manager.save_last_session_filename(&session_filename)?;
                UI::display_exit_message();
                // break;
            }
            Err(ReadlineError::Eof) => {
                let session_filename = session_manager.new_session_filename();
                session_manager.save_chat_to_session(&session_filename, &messages)?;
                session_manager.save_last_session_filename(&session_filename)?;
                UI::display_exit_message();
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    Ok(())
}
