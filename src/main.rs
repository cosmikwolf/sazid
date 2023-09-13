use std::path::PathBuf;

use clap::Parser;
use sazid::{types::*, utils::initialize_tracing};
use sazid::ui::UI;
use tracing::{self, event, Level};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>>  {
    initialize_tracing().unwrap();
    tracing::debug!("sazid start main");
    let opts: Opts = Opts::parse();
    let settings = GPTSettings::load(PathBuf::from("Settings.toml"));

    // Initialize the SessionManager.
    let session_data = match &opts.new {
        true => {
            Session::new(settings, opts.include_functions)
        }
        false => {
            match &opts.continue_session {
                Some(session_id) => {
                    Session::load_session_by_id(session_id.clone())
                }
                None => {
                    Session::load_last_session()
                }
            }
        }
    };
    // Initialize the user interface
    let mut ui = UI::init(session_data, opts.clone());

    // Handle model selection based on CLI flag
    if let Some(model_name) = &opts.model {
        // In a real-world scenario, you would set the selected model in the session manager or GPT connector
        ui.display_general_message(format!("Using model: {}", model_name));
    }

    ui.run_interface_loop(opts.batch).unwrap();
    
    Ok(())
}
