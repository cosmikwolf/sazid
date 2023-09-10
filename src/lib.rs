// The primary purpose of this file will be to expose the modules for the main.rs and integration tests.
#[macro_use]
extern crate lazy_static;

pub mod gpt_connector;
pub mod gpt_commands;
pub mod session_manager;
pub mod ui;
pub mod chunkifier;
pub mod pdf_extractor;
pub mod errors;
pub mod utils;
pub mod types;
pub mod consts;