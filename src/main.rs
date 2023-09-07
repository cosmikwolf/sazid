use clap::Parser;
use sazid::types::*;
use sazid::ui::UI;
use tokio::runtime::Runtime;
use toml;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new().unwrap();

    let opts: Opts = Opts::parse();
    let settings: GPTSettings =
        toml::from_str(std::fs::read_to_string("Settings.toml").unwrap().as_str()).unwrap();

    // Initialize the SessionManager.

    let session_data: Option<Session> = None;

    let session_manager = SessionManager::new(settings, session_data, rt);

    // Initialize the user interface
    let mut ui = UI::init(session_manager, opts.clone());

    // Handle model selection based on CLI flag
    if let Some(model_name) = &opts.model {
        // In a real-world scenario, you would set the selected model in the session manager or GPT connector
        ui.display_general_message(format!("Using model: {}", model_name));
    }

    ui.run_interface_loop(opts.batch).unwrap();
    /*
    // // Handle ingesting text from stdin
    // match opts.stdin {
    //     Some(stdin) => {
    //         rt.block_on(async {
    //             let chunks = Chunkifier::chunkify_input(
    //                 &stdin.to_str().unwrap().to_string(),
    //                 session_manager.session_data.model.token_limit as usize,
    //             )
    //             .unwrap();
    //             // iterate through chunks and use ui read_stdin to display them
    //             for chunk in chunks.clone() {
    //                 ui.read_stdin(chunk);
    //             }
    //             session_manager.handle_ingest(chunks).await
    //         }).unwrap()
    //     }
    //     None => {}
    // }

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
        ui.display_chat_history(&session_manager.get_chat_history());
    }

    loop {
        match ui.read_input("You: ") {
            Ok(input) => {
                let input = input.unwrap();
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
                        ui.display_exit_message();
                        return Ok(());
                    }
                    let messages = session_manager.construct_request_and_cache(vec![input.to_string()]);
                    match rt.block_on(async { session_manager.send_request(&mut ui, messages).await }) {
                        Ok(response) => {
                            let usage = response.usage.unwrap();
                            ui.display_debug_message(format!("created: {:?}\tmodel: {:?}\tfinish_reason:{:?}\tprompt_tokens:{:?}\tcompletion_tokens:{:?}\ttotal_tokens:{:?}\t", response.created, response.model, response.choices[0].finish_reason, usage.prompt_tokens, usage.completion_tokens, usage.total_tokens));

                            session_manager.save_session()?;
                        }
                        Err(error) => {
                            // Displaying the error to the user
                            ui.display_chat_message(Role::System, format!("Error: {}", error));

                            // Logging the request and the error
                            // NOTE: We'll need an instance or reference to the session manager here to call save_chat_to_session
                            // session_manager.save_chat_to_session("error_log.json", &vec![input.to_string()], &None).expect("Failed to save error log");
                        }
                    }
                }
            }
            Err(error) => {
                ui.display_error_message(format!("Error sending request to GPT: {:?}", error));
                // session_manager.save_last_session_file_path();
                ui.display_exit_message();
                break
            }
        }
        println!("loop end")
    }
    */
    Ok(())
}
