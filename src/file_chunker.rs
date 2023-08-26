use std::fs;
use std::path::Path;
use pdf::file::File as PdfFile;

pub enum FileType {
    Text,
    PDF,
    HTML,
    Rust,
    TOML,
    Unknown,
}

pub struct ChunkResult {
    pub chunk: String,
    pub total_chunks: usize,
}

pub fn determine_file_type(filename: &str) -> FileType {
    match Path::new(filename).extension().and_then(|s| s.to_str()) {
        Some("txt") => FileType::Text,
        Some("pdf") => FileType::PDF,
        Some("html") | Some("htm") => FileType::HTML,
        Some("rs") => FileType::Rust,
        Some("toml") => FileType::TOML,
        _ => FileType::Unknown,
    }
}

pub fn chunk_file(filename: &str, index: usize, chunk_size: usize) -> Result<ChunkResult, &'static str> {
    match determine_file_type(filename) {
        FileType::Text => chunk_text_file(filename, index, chunk_size),
        FileType::PDF => chunk_pdf_file(filename, index, chunk_size),
        _ => Err("Unsupported file type"),
    }
}

fn chunk_text_file(filename: &str, index: usize, chunk_size: usize) -> Result<ChunkResult, &'static str> {
    let content = fs::read_to_string(filename).map_err(|_| "Failed to read the file")?;
    let start = index * chunk_size;
    let end = (index + 1) * chunk_size;
    let total_chunks = (content.len() + chunk_size - 1) / chunk_size;

    if start >= content.len() {
        return Err("Index out of bounds");
    }

    let chunk = &content[start..end.min(content.len())];
    Ok(ChunkResult {
        chunk: chunk.to_string(),
        total_chunks,
    })
}

fn chunk_pdf_file(filename: &str, index: usize, chunk_size: usize) -> Result<ChunkResult, &'static str> {
    let pdf = PdfFile::<Vec<u8>>::open(filename).map_err(|_| "Failed to open the PDF file")?;
    let mut content = String::new();

    for page in pdf.pages() {
        content.push_str(&page.contents().unwrap_or_default());
    }

    chunk_text_file(&content, index, chunk_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_file_type() {
        assert_eq!(determine_file_type("sample.txt"), FileType::Text);
        assert_eq!(determine_file_type("sample.pdf"), FileType::PDF);
        assert_eq!(determine_file_type("sample.html"), FileType::HTML);
        assert_eq!(determine_file_type("sample.rs"), FileType::Rust);
        assert_eq!(determine_file_type("sample.toml"), FileType::TOML);
        assert_eq!(determine_file_type("sample.unknown"), FileType::Unknown);
    }

    #[test]
    fn test_chunk_text_file() {
        // Assuming a sample.txt file exists with content "Hello, World!"
        let result = chunk_text_file("sample.txt", 0, 5).unwrap();
        assert_eq!(result.chunk, "Hello");
    }

    // Additional tests for chunk_pdf_file and other functionalities can be added here.
}
