use clap::Parser;

use crate::utils::version;

#[derive(Parser, Debug, Clone)]
#[command(author, version = version(), about)]
pub struct Cli {
  #[arg(
    short = 'l',
    long = "list-embeddings",
    value_name = "bool",
    help = "list embeddings loaded into the database",
    default_value_t = false
  )]
  pub list_embeddings: bool,

  #[arg(
    short = 's',
    long = "search-embeddings",
    value_name = "STRING",
    help = "perform a vector similarity search on all embeddings"
  )]
  pub search_embeddings: Option<String>,

  #[arg(
    short = 'c',
    long = "code-embeddings",
    value_name = "STRING",
    help = "parse a source file using treesitter and load ast into vector database"
  )]
  pub parse_source_embeddings: Option<String>,

  #[arg(
    short = 'f',
    long = "textfile",
    value_name = "STRING",
    help = "read a text file, generate embeddings, and load into vector database"
  )]
  pub add_text_file_embeddings: Option<String>,

  #[arg(short, long, value_name = "BOOL", help = "delete all embeddings from the database")]
  pub delete_all_embeddings: bool,

  #[arg(
    short = 't',
    long = "text",
    value_name = "STRING",
    help = "read text argument, generate embeddings, and load into vector database"
  )]
  pub add_text_embeddings: Option<String>,

  #[arg(
    short = 'i',
    long,
    value_name = "FLOAT",
    help = "Tick rate, i.e. number of ticks per second",
    default_value_t = 1.00
  )]
  pub tick_rate: f64,

  #[arg(
    short = 'r',
    long,
    value_name = "FLOAT",
    help = "Frame rate, i.e. number of frames per second",
    default_value_t = 20.0
  )]
  pub frame_rate: f64,

  #[arg(short = 'a', long, help = "Connect to localhost LLVM API endpoint", default_value_t = false)]
  pub local_api: bool,
}
