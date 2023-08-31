use async_openai::types::ChatCompletionRequestMessage;
use async_openai::types::Role;
use clap::Parser;
use config::Config;
use rustyline::error::ReadlineError;
use sazid::gpt_connector::GPTConnector;
use sazid::session_manager::SessionManager;
use sazid::ui::UI;
use sazid::utils::generate_session_id;
use std::ffi::OsString;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();

    let settings = Config::builder()
        // Add in `./Settings.toml`
        .add_source(config::File::with_name("Settings"))
        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
        .build()
        .unwrap();

    let gpt = GPTConnector::new(settings).await;

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

    // Declare the SessionManager object.
    let mut session_manager: SessionManager;

    // Check if the `--new` flag is provided.
    if opts.new {
        // Instantiate a new SessionManager for a new session.
        let session_id = generate_session_id();
        session_manager = SessionManager::new(session_id, &gpt);
    } else {
        // Check if a specific session is provided via the `--continue` flag.
        match opts.continue_session {
            Some(session_file) => {
                // Load the provided session.
                session_manager = SessionManager::load_session(&session_file, &gpt);
            }
            None => {
                // Check if there's a last session.
                if let Some(last_session) = SessionManager::load_last_session_filename() {
                    // Load the last session.
                    session_manager = SessionManager::load_session(&last_session.to_str().unwrap(), &gpt);
                } else {
                    // No last session available. Instantiate a new SessionManager for a new session.
                    let session_id = generate_session_id();
                    session_manager = SessionManager::new(session_id, &gpt);
                }
            }
        }
    }

    if let Some(path) = &opts.ingest {
        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()?
            .block_on(session_manager.handle_ingest(&path.to_string_lossy().to_string()))?;
    }

    UI::display_startup_message();

    for message in &session_manager.session_data.requests {
        UI::display_message(message.role.clone(), message.content.clone().unwrap_or_default());
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
                        session_manager.save_last_session_filename();
                        UI::display_exit_message();
                        break;
                    }
                    let user_message = ChatCompletionRequestMessage {
                        role: Role::User,
                        content: Some(input.to_string()),
                        function_call: None, // If you have appropriate data, replace None
                        name: None,          // If you have appropriate data, replace None
                    };
                    session_manager.session_data.requests.push(user_message);

                    match tokio::runtime::Builder::new_current_thread()
                        .enable_io()
                        .enable_time()
                        .build()?
                        .block_on(gpt.send_request(vec![input.to_string()]))
                    {
                        Ok(response) => {
                            for choice in &response.choices {
                                UI::display_message(
                                    choice.message.role.clone(),
                                    choice.message.content.clone().unwrap_or_default(),
                                );
                            }
                            session_manager.session_data.responses.push(response);
                            session_manager.save_session()?;
                        }
                        Err(error) => {
                            // Displaying the error to the user
                            UI::display_message(Role::System, format!("Error: {}", error));

                            // Logging the request and the error
                            // NOTE: We'll need an instance or reference to the session manager here to call save_chat_to_session
                            // session_manager.save_chat_to_session("error_log.json", &vec![input.to_string()], &None).expect("Failed to save error log");
                        }
                    }
                }
            }

            Err(ReadlineError::Interrupted) => {
                // session_manager.save_chat_to_session(&session_filename, &messages)?;
                session_manager.save_last_session_filename();
                UI::display_exit_message();
                // break;
            }
            Err(ReadlineError::Eof) => {
                // session_manager.save_chat_to_session(&session_filename, &messages)?;
                session_manager.save_last_session_filename();
                UI::display_exit_message();
                break;
            }
            Err(e) => {
                println!("Error sending request to GPT: {:?}", e);
            }
        }
    }
    Ok(())
}
