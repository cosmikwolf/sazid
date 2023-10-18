// #![allow(dead_code)]
// #![allow(unused_imports)]
// #![allow(unused_variables)]

extern crate lazy_static;

use clap::Parser;
use color_eyre::eyre::Result;

use sazid::{
  app::App,
  cli::Cli,
  trace_dbg,
  utils::{initialize_logging, initialize_panic_handler},
};

async fn tokio_main() -> Result<()> {
  initialize_logging()?;

  initialize_panic_handler()?;
  trace_dbg!("app start");
  let args = Cli::parse();
  let mut app = App::new(args.tick_rate, args.frame_rate)?;
  app.run().await?;

  Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
  if let Err(e) = tokio_main().await {
    eprintln!("{} error: Something went wrong", env!("CARGO_PKG_NAME"));
    Err(e)
  } else {
    Ok(())
  }
}
