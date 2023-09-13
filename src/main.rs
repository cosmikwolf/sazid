use std::path::PathBuf;

use clap::Parser;
use sazid::{types::*, utils::initialize_tracing};
use sazid::ui::UI;
use tracing::{self, event, Level};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>>  {
    initialize_tracing().unwrap();
    tracing::debug!("sazid start main");
 
    // Handle model selection based on CLI flag
    if let Some(model_name) = &opts.model {
        // In a real-world scenario, you would set the selected model in the session manager or GPT connector
        ui.display_general_message(format!("Using model: {}", model_name));
    }

    ui.run_interface_loop(opts.batch).unwrap();
    
    Ok(())
}
