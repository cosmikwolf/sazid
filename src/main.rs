use async_openai::types::ChatCompletionRequestMessage;
use async_openai::types::Role;
use clap::Parser;
use futures::TryFutureExt;
use rustyline::error::ReadlineError;
use sazid::gpt_connector::GPTConnector;
use sazid::gpt_connector::GPTSettings;
use sazid::session_manager::SessionManager;
use sazid::ui::UI;
use sazid::utils::generate_session_id;
use std::path::PathBuf;
use toml;
use sazid::ui::Opts;
use tokio::runtime::Runtime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt  = Runtime::new()?;

    let opts: Opts = Opts::parse();
    let settings: GPTSettings = toml::from_str(std::fs::read_to_string("Settings.toml").unwrap().as_str()).unwrap();
    
    let gpt: GPTConnector = rt.block_on( GPTConnector::new(&settings));

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
                session_manager = SessionManager::load_session_from_file(PathBuf::from(&session_file), &gpt);
            }
            None => {
                // Check if there's a last session.
                if let Some(last_session) = SessionManager::load_last_session_file_path() {
                    // Load the last session.
                    session_manager = SessionManager::load_session_from_file(last_session, &gpt);
                } else {
                    // No last session available. Instantiate a new SessionManager for a new session.
                    let session_id = generate_session_id();
                    session_manager = SessionManager::new(session_id, &gpt);
                }
            }
        }
    }

    if let Some(path) = &opts.ingest {
        rt.block_on(async{session_manager.handle_ingest(&path.to_string_lossy().to_string()).await}).unwrap();
    }

    UI::display_startup_message();

    for message in &session_manager.session_data.requests {
        UI::display_message(message.role.clone(), message.content.clone().unwrap_or_default());
    }
    for message in &session_manager.session_data.responses {
        message.choices.clone().into_iter().for_each(|choice| {
            UI::display_message(choice.message.role.clone(), choice.message.content.clone().unwrap_or_default());
        });
    }

    loop {
        match UI::read_input("You: ") {
            Ok(input) => {
                let input = input.trim();

                if input.starts_with("ingest ") {
                    let filepath = input.split_whitespace().nth(1).unwrap_or_default();
                    rt.block_on(async{session_manager.handle_ingest(&filepath.to_string()).await}).unwrap();
                } else {
                    if input == "exit" || input == "quit" {
                        session_manager.save_session().unwrap();
                        UI::display_exit_message();
                        return Ok(());
                    }
                    let user_message = ChatCompletionRequestMessage {
                        role: Role::User,
                        content: Some(input.to_string()),
                        function_call: None, // If you have appropriate data, replace None
                        name: None,          // If you have appropriate data, replace None
                    };
                    session_manager.session_data.requests.push(user_message);
                    
                    match rt.block_on(async{ gpt.send_request(
                        gpt.construct_request_message_array(Role::User, vec![input.to_string()])    
                    ).await
                })
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
                session_manager.save_last_session_file_path();
                UI::display_exit_message();
                break;
            }
            Err(ReadlineError::Eof) => {
                // session_manager.save_chat_to_session(&session_filename, &messages)?;
                session_manager.save_last_session_file_path();
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