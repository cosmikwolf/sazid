use clap::Parser;

use crate::utils::version;

#[derive(Parser, Debug)]
#[command(author, version = version(), about)]
pub struct Cli {
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
    default_value_t = 15.0
  )]
  pub frame_rate: f64,

  #[arg(short, long, help = "Connect to localhost LLVM API endpoint", default_value_t = false)]
  pub local_api: bool,
}
