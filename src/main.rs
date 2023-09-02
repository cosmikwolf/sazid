use async_openai::types::Role;
use clap::Parser;
use rustyline::error::ReadlineError;
use sazid::chunkifier::Chunkifier;
use sazid::gpt_connector::GPTSettings;
use sazid::session_manager::Session;
use sazid::session_manager::SessionManager;
use sazid::ui::Opts;
use sazid::ui::UI;
use std::fs;
use std::path::PathBuf;
use tokio::runtime::Runtime;
use toml;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new()?;

    let opts: Opts = Opts::parse();
    let settings: GPTSettings =
        toml::from_str(std::fs::read_to_string("Settings.toml").unwrap().as_str()).unwrap();

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

    let mut session_data: Option<Session> = None;
    
    // Check if the `--new` flag is provided (or not).
    // If so, initiate a new SessionManager for a new session.
    // for this nothing needs to be done as session_data is already None
    if !opts.new {
        // Check if a specific session is provided via the `--continue` flag.
        match opts.continue_session {
            Some(session_file) => {
                // Load the provided session.
                let session_path = PathBuf::from(&session_file);
                if !session_path.exists() {
                    UI::display_error_message(format!(
                        "Session file not found: {}",
                        session_path.display()
                    ));
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("Session file not found: {}", session_path.display()),
                    )));
                } else {
                    let session_file_path = PathBuf::from(&session_file);
                    let data = fs::read_to_string(session_file_path).unwrap();
                    session_data = Some(serde_json::from_str(&data).unwrap());
                }
            }
            None => {
                // Check if there's a last session.
                if let Some(last_session) = SessionManager::get_last_session_file_path() {
                    // Load the last session.
                    if !last_session.exists() {
                        UI::display_error_message(format!(
                            "Session file not found: {}",
                            last_session.display()
                        ));
                    } else {
                        let session_file_path = PathBuf::from(&last_session);
                        let data = fs::read_to_string(session_file_path).unwrap();
                        session_data = Some(serde_json::from_str(&data).unwrap());
                    }
                }
            }
        }
    }
    // Initialize the SessionManager.
    let mut session_manager = rt.block_on(async { SessionManager::new(settings, session_data).await });
    
    // Handle ingesting text from stdin
    match opts.stdin {
        Some(stdin) => {
            rt.block_on(async {
                let chunks = Chunkifier::chunkify_input(
                    &stdin.to_str().unwrap().to_string(),
                    session_manager.session_data.model.token_limit as usize,
                )
                .unwrap();
                // iterate through chunks and use ui read_stdin to display them
                for chunk in chunks.clone() {
                    UI::read_stdin(chunk);
                } 
                session_manager.handle_ingest(chunks).await
            }).unwrap()
        }
        None => {}
    }
    // Display the welcome message.
    UI::display_startup_message();

    if let Some(path) = &opts.ingest {
        rt.block_on(async {
            let chunks = Chunkifier::chunkify_input(
                &path.to_string_lossy().to_string(),
                session_manager.session_data.model.token_limit as usize,
            )
            .unwrap();
            session_manager.handle_ingest(chunks).await
        })
        .unwrap();
    }

    // Display chat history if available
    if !session_manager.session_data.interactions.is_empty() {
        UI::display_chat_history(&session_manager.get_chat_history());
    }

    loop {
        match UI::read_input("You: ") {
            Ok(input) => {
                let input = input.trim();
                if input.starts_with("ingest ") {
                    let path = input.split_whitespace().nth(1).unwrap_or_default();
                    rt.block_on(async {
                        let chunks = Chunkifier::chunkify_input(
                            &path.to_string(),
                            session_manager.session_data.model.token_limit as usize,
                        )
                        .unwrap();
                        session_manager.handle_ingest(chunks).await
                    })
                    .unwrap();
                } else {
                    if input == "exit" || input == "quit" {
                        session_manager.save_session().unwrap();
                        UI::display_exit_message();
                        return Ok(());
                    }
                    let messages = session_manager.construct_request_and_cache(vec![input.to_string()]);
                    match rt.block_on(async { session_manager.send_request(messages).await }) {
                        Ok(response) => {
                            let usage = response.usage.unwrap();
                            UI::display_debug_message(format!("created: {:?}\tmodel: {:?}\tfinish_reason:{:?}\tprompt_tokens:{:?}\tcompletion_tokens:{:?}\ttotal_tokens:{:?}\t", response.created, response.model, response.choices[0].finish_reason, usage.prompt_tokens, usage.completion_tokens, usage.total_tokens));
                           
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
