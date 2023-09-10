use std::path::PathBuf;

use clap::Parser;
use sazid::types::*;
use sazid::ui::UI;
use tokio::runtime::Runtime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new().unwrap();

    let opts: Opts = Opts::parse();
    let settings = GPTSettings::load(PathBuf::from("Settings.toml"));

    // Initialize the SessionManager.
    let session_data: Option<Session> = None;
    let session_manager = SessionManager::new(settings, opts.include_functions, session_data, rt);

    // Initialize the user interface
    let mut ui = UI::init(session_manager, opts.clone());

    // Handle model selection based on CLI flag
    if let Some(model_name) = &opts.model {
        // In a real-world scenario, you would set the selected model in the session manager or GPT connector
        ui.display_general_message(format!("Using model: {}", model_name));
    }

    ui.run_interface_loop(opts.batch).unwrap();
    
    Ok(())
}
