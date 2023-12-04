use clap::Parser;

use crate::utils::version;

#[derive(Parser, Debug)]
#[command(author, version = version(), about)]
pub struct Cli {
  #[arg(
    short,
    long = "list-embeddings",
    value_name = "bool",
    help = "list embeddings loaded into the database",
    default_value_t = false
  )]
  pub list_embeddings: bool,

  #[arg(
    short,
    long = "parse-source-embeddings",
    value_name = "STRING",
    help = "parse a source file using treesitter and load ast into vector database"
  )]
  pub parse_source_embeddings: Option<String>,

  #[arg(
    short,
    long = "load-text-file-embeddings",
    value_name = "STRING",
    help = "read a text file, generate embeddings, and load into vector database"
  )]
  pub load_text_file_embeddings: Option<String>,

  #[arg(
    short,
    long = "load-text-embeddings",
    value_name = "STRING",
    help = "read text argument, generate embeddings, and load into vector database"
  )]
  pub load_text_embeddings: Option<String>,

  #[arg(
    short,
    long,
    value_name = "FLOAT",
    help = "Tick rate, i.e. number of ticks per second",
    default_value_t = 1.00
  )]
  pub tick_rate: f64,

  #[arg(
    short,
    long,
    value_name = "FLOAT",
    help = "Frame rate, i.e. number of frames per second",
    default_value_t = 20.0
  )]
  pub frame_rate: f64,

  #[arg(short, long, help = "Connect to localhost LLVM API endpoint", default_value_t = false)]
  pub local_api: bool,
}
