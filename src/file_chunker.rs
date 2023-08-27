use std::fs;
use std::io::Read;
use pdf::file::File as PdfFile;

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
    let mut file = fs::File::open(file_path).expect("Unable to open the PDF file.");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).expect("Unable to read the PDF file.");

    let pdf_file = PdfFile::open(file_path).expect("Unable to parse the PDF file.");
    let total_indexes = pdf_file.num_pages() as usize;

    if index >= total_indexes {
        (String::from("Index out of bounds."), total_indexes)
    } else {
        let page = pdf_file.get_page(index as u32).expect("Unable to get the PDF page.");
        let content = page.get_text().expect("Unable to extract text from the PDF page.");
        (content, total_indexes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text_file() {
        let content = "This is a sample text for testing.";
        let filename = "test.txt";
        fs::write(filename, content).unwrap();
        let (chunk, total) = chunk_file(filename, 1).unwrap();
        assert_eq!(chunk, "This is a sample text for testing.");
        assert_eq!(total, 1);
    }

    #[test]
    fn test_chunk_pdf_file() {
        // Placeholder test for PDF chunking
        assert_eq!(2 + 2, 4);  // This is a placeholder and should be replaced with actual test logic for PDFs.
    }
}