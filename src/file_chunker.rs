use std::fs;
use lopdf::{Document};

pub enum FileType {
    Text,
    Pdf,
    Other,
}

pub fn chunk_file(file_path: &str, index: usize) -> (String, usize) {
    let file_type = determine_file_type(file_path);
    match file_type {
        FileType::Text => chunk_text_file(file_path, index),
        FileType::Pdf => chunk_pdf_file(file_path, index),
        FileType::Other => (String::from("Unsupported file type."), 0),
    }
}

fn determine_file_type(file_path: &str) -> FileType {
    if file_path.ends_with(".txt") {
        FileType::Text
    } else if file_path.ends_with(".pdf") {
        FileType::Pdf
    } else {
        FileType::Other
    }
}

fn chunk_text_file(file_path: &str, index: usize) -> (String, usize) {
    let content = fs::read_to_string(file_path).expect("Unable to read the file.");
    let chunks: Vec<&str> = content.split_whitespace().collect();
    let total_indexes = chunks.len();

    if index >= total_indexes {
        (String::from("Index out of bounds."), total_indexes)
    } else {
        (chunks[index].to_string(), total_indexes)
    }
}

fn chunk_pdf_file(file_path: &str, index: usize) -> (String, usize) {
    let doc = Document::load(file_path).expect("Unable to load the PDF file.");
    let total_indexes = doc.get_pages().len() as usize;

    if index >= total_indexes {
        (String::from("Index out of bounds."), total_indexes)
    } else {
        // TODO: Retrieve the page using the object ID
        let content = extract_text_from_page(&doc, &doc.get_pages()[index]);
        (content, total_indexes)
    }
}

fn extract_text_from_page(doc: &Document, page: &lopdf::Dictionary) -> String {
    let resources = page.get(b"Resources").and_then(|obj| obj.as_dict()).unwrap();
    let content = doc.get_page_content(&doc.get_pages()[index]).unwrap();
    String::from_utf8_lossy(&content).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_chunk_text_file() {
        let content = "This is a sample text for testing.";
        let filename = "test.txt";
        fs::write(filename, content).unwrap();
        let (chunk, total) = chunk_file(filename, 1);
        assert_eq!(chunk, "This is a sample text for testing.");
        assert_eq!(total, 1);
    }

    // TODO: Add tests for PDF chunking
}
