use crate::types::Model;
pub const MAX_FUNCTION_CALL_DEPTH: u32 = 3;
pub const CHUNK_TOKEN_LIMIT: u32 = 4096u32;
lazy_static! {
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
