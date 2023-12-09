// #![allow(dead_code)]
// #![allow(unused_imports)]
// #![allow(unused_variables)]

extern crate lazy_static;

use clap::Parser;
use color_eyre::eyre::Result;

use sazid::{
  app::{embeddings::EmbeddingsManager, errors::SazidError, App},
  cli::Cli,
  config::Config,
  trace_dbg,
  utils::{initialize_logging, initialize_panic_handler},
};

async fn tokio_main() -> Result<(), SazidError> {
  initialize_logging().map_err(SazidError::LoggingError)?;
  initialize_panic_handler().map_err(SazidError::PanicHandlerError)?;
  trace_dbg!("app start");
  let args = Cli::parse();
  let config = Config::new(args.local_api).unwrap();
  let db_config = "host=localhost user=docker password=docker database=sazid";

  match EmbeddingsManager::run(db_config, args.clone(), config.clone()).await {
    Ok(Some(output)) => {
      println!("{}", output);
      Ok(())
    },
    Ok(None) => {
      println!("No output");
      let mut app = App::new(args.tick_rate, args.frame_rate, config).unwrap();
      app.run().await.unwrap();
      Ok(())
    },
    Err(e) => {
      eprintln!("{} error: {}", env!("CARGO_PKG_NAME"), e);
      Err(e)
    },
  }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<(), SazidError> {
  if let Err(e) = tokio_main().await {
    eprintln!("{} error: Something went wrong", env!("CARGO_PKG_NAME"));
    Err(e)
  } else {
    Ok(())
  }
}
