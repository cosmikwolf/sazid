use serde::{Deserialize, Serialize};

pub mod color_math;
pub mod consts;
pub mod database;
pub mod errors;
pub mod functions;
pub mod gpt_interface;
pub mod helpers;
pub mod lsp;
pub mod markdown;
pub mod messages;
pub mod request_validation;
pub mod session_config;
pub mod tools;
pub mod treesitter;
pub mod types;

#[derive(
  Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub enum Mode {
  #[default]
  Home,
}
