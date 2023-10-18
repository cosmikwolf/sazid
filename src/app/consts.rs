use crate::app::types::Model;
use lazy_static::lazy_static;
use std::path::PathBuf;

pub const MAX_FUNCTION_CALL_DEPTH: u32 = 0;
pub const CHUNK_TOKEN_LIMIT: u32 = 4096u32;

pub const SESSIONS_DIR: &str = "data/sessions";
pub const INGESTED_DIR: &str = "data/ingested";

lazy_static! {
    // model constants
    pub static ref GPT3_TURBO_16K: Model = Model {
        name: "gpt-3.5-turbo-16k".to_string(),
        endpoint: "https://api.openai.com/v1/completions".to_string(),
        token_limit: 16384,
    };
    pub static ref GPT3_TURBO: Model = Model {
        name: "gpt-3.5-turbo".to_string(),
        endpoint: "https://api.openai.com/v1/completions".to_string(),
        token_limit: 4097,
    };
    pub static ref GPT4: Model = Model {
        name: "gpt-4".to_string(),
        endpoint: "https://api.openai.com/v1/completions".to_string(),
        token_limit: 8192,
    };
    // logging constants
    pub static ref PROJECT_NAME: String = env!("CARGO_CRATE_NAME").to_uppercase().to_string();
    pub static ref DATA_FOLDER: Option<PathBuf> =
        std::env::var(format!("{}_DATA", PROJECT_NAME.clone()))
            .ok()
            .map(PathBuf::from);
    pub static ref CONFIG_FOLDER: Option<PathBuf> =
        std::env::var(format!("{}_CONFIG", PROJECT_NAME.clone()))
            .ok()
            .map(PathBuf::from);
    pub static ref GIT_COMMIT_HASH: String =
        std::env::var(format!("{}_GIT_INFO", PROJECT_NAME.clone()))
            .unwrap_or_else(|_| String::from("Unknown"));
    pub static ref LOG_LEVEL: String =
        std::env::var(format!("{}_LOG_LEVEL", PROJECT_NAME.clone())).unwrap_or_default();
    pub static ref LOG_FILE: String = format!("{}.log", env!("CARGO_PKG_NAME").to_lowercase());
}
pub static PDF_IGNORE: &[&str] = &[
  "Length",
  "BBox",
  "FormType",
  "Matrix",
  "Resources",
  "Type",
  "XObject",
  "Subtype",
  "Filter",
  "ColorSpace",
  "Width",
  "Height",
  "BitsPerComponent",
  "Length1",
  "Length2",
  "Length3",
  "PTEX.FileName",
  "PTEX.PageNumber",
  "PTEX.InfoDict",
  "FontDescriptor",
  "ExtGState",
  "Font",
  "MediaBox",
  "Annot",
];
