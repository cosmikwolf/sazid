pub mod application;
pub mod config;
extern crate lazy_static;

use std::env;

use async_openai::config::OpenAIConfig;
use clap::Parser;
use color_eyre::eyre::Result;

use sazid::{
  app::{
    database::{data_manager::DataManager, data_models::EmbeddingModel},
    errors::SazidError,
    App,
  },
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
  let api_key: String =
    env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
  let openai_config = OpenAIConfig::new()
    .with_api_key(api_key)
    .with_org_id("org-WagBLu0vLgiuEL12dylmcPFj");
  let mut embeddings_manager =
    DataManager::new(EmbeddingModel::Ada002(openai_config)).await?;

  match embeddings_manager.run_cli(args.clone()).await {
    Ok(Some(output)) => {
      println!("{}", output);
      Ok(())
    },
    Ok(None) => {
      println!("No output");
      let mut app =
        App::new(args.tick_rate, args.frame_rate, config, embeddings_manager)
          .await
          .unwrap();
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
