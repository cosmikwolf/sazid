use crate::types::Model;

lazy_static! {
    pub static ref GPT3_TURBO: Model = Model {
        name: "gpt-3.5-turbo".to_string(),
        endpoint: "https://api.openai.com/v1/completions".to_string(),
        token_limit: 4096,
    };
    pub static ref GPT4: Model = Model {
        name: "gpt-4".to_string(),
        endpoint: "https://api.openai.com/v1/completions".to_string(),
        token_limit: 4096,
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
