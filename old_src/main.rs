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




// ANCHOR: all
pub mod runner;

pub mod action;

pub mod components;

pub mod config;

pub mod tui;

pub mod utils;

use crate::{
  runner::Runner,
  utils::{initialize_logging, initialize_panic_handler, version},
};
use clap::Parser;
use color_eyre::eyre::Result;

//// ANCHOR: args
// Define the command line arguments structure

#[tokio::main]
async fn main() -> Result<()> {
  initialize_logging()?;
  initialize_panic_handler()?;
  let tick_rate = 250;
  let render_tick_rate = 100;
  let mut runner = Runner::new((tick_rate, render_tick_rate))?;
  runner.run().await?;

  Ok(())
}
// ANCHOR_END: all
