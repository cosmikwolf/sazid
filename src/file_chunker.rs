use crate::pdf_extractor::PdfText;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// Chunk the given file based on its type (PDF, text, etc.).
pub fn chunk_file<P: AsRef<Path>>(file_path: P, index: usize) -> (String, usize) {
    if is_binary_file(&file_path) {
        if is_pdf_file(&file_path) {
            return chunk_pdf_file(file_path, index);
        } else {
            return (
                String::from("The provided file appears to be binary and cannot be processed."),
                0,
            );
        }
    }

    let ext = file_path.as_ref().extension().and_then(|s| s.to_str());
    match ext {
        Some("pdf") => chunk_pdf_file(file_path, index),
        _ => chunk_text_file(file_path, index), // Default to text file chunking
    }
}

/// Check if the given file is a PDF.
fn is_pdf_file<P: AsRef<Path>>(file_path: P) -> bool {
    file_path.as_ref().extension().and_then(|s| s.to_str()) == Some("pdf")
}

/// Check if the given file appears to be a binary file.
fn is_binary_file<P: AsRef<Path>>(file_path: P) -> bool {
    let mut file = File::open(file_path).expect("Failed to open file.");
    let mut buffer = [0u8; 1024];
    let n = file.read(&mut buffer).expect("Failed to read file.");

    // Check for a significant number of non-text bytes (e.g., outside ASCII range)
    buffer[..n]
        .iter()
        .filter(|&&b| b < 7 || (b > 14 && b < 32))
        .count()
        > n / 8
}

/// Chunk a given PDF file and retrieve the content of the specified page.
fn chunk_pdf_file<P: AsRef<Path>>(file_path: P, index: usize) -> (String, usize) {
    let pdf_text = PdfText::from_pdf(file_path).expect("Failed to extract text from PDF.");
    let total_pages = pdf_text.total_pages();

    if index == 0 || index > total_pages {
        (String::from("Index out of bounds."), total_pages)
    } else {
        let content = pdf_text
            .get_page_text(index as u32)
            .map(|lines| lines.join("\n"))
            .unwrap_or_default();
        (content, total_pages)
    }
}

/// Chunk a given text file line by line.
fn chunk_text_file<P: AsRef<Path>>(file_path: P, index: usize) -> (String, usize) {
    let file = File::open(file_path).expect("Failed to open file.");
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

    if index == 0 || index > lines.len() {
        (String::from("Index out of bounds."), lines.len())
    } else {
        (lines[index - 1].clone(), lines.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use std::path::{Path, PathBuf};

    fn create_sample_pdf(dir: &Path) -> Result<PathBuf, std::io::Error> {
        use lopdf::{Document, Dictionary, Object, Stream};
            
        let mut doc = Document::new();
        
        let pages_id = doc.new_object_id();
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", "Font");
        font_dict.set("Subtype", "Type1");
        font_dict.set("BaseFont", "Courier");
        let font_id = doc.add_object(Object::Dictionary(font_dict));
            
        let content = "BT /F1 24 Tf 100 600 Td (Hello, PDF!) Tj ET";
        let content_stream = Stream::new(Dictionary::new(), content.as_bytes().to_vec());
        let content_id = doc.add_object(Object::Stream(content_stream));
        
        let mut page_dict = Dictionary::new();
        page_dict.set("Type", "Page");
        page_dict.set("Parent", pages_id);
        let mut resource_dict = Dictionary::new();
        resource_dict.set("Font", font_id);
        page_dict.set("Resources", resource_dict);
        page_dict.set("Contents", content_id);
        let page_id = doc.add_object(Object::Dictionary(page_dict));
        
        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", "Pages");
        pages_dict.set("Kids", vec![page_id.into()]);
        pages_dict.set("Count", 1);
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
        
        let catalog = doc.catalog().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let root_id = doc.add_object(Object::Dictionary(catalog.clone()));
        doc.trailer.set("Root", root_id);
            
        let pdf_path = dir.join("sample.pdf");
        doc.save(pdf_path.clone()).expect("Unable to save PDF.");
        
        Ok(pdf_path)
    }
    
    #[test]
    fn test_chunk_pdf() {
        let dir = tempdir().expect("Unable to create temporary directory.");
        let pdf_path_result = create_sample_pdf(dir.path());
        if let Ok(pdf_path) = pdf_path_result {
            let result = chunk_file(&pdf_path, 0);
            assert_eq!(result.0, "Hello, PDF!");
        } else {
            panic!("Failed to create sample PDF.");
        }
    }

    #[test]
    fn test_chunk_binary_file() {
        let dir = tempdir().expect("Unable to create temporary directory.");
        let binary_file_path = dir.path().join("binary_test_file.bin");

        let mut file = File::create(&binary_file_path).expect("Unable to create file.");
        file.write_all(&[0u8, 1, 2, 3, 4, 255])
            .expect("Unable to write to file.");

        let result = chunk_file(&binary_file_path, 0);
        assert_eq!(result.0, "The provided file appears to be binary and cannot be processed.");
    }

    #[test]
    fn test_chunk_text_file() {
        let dir = tempdir().expect("Unable to create temporary directory.");
        let text_file_path = dir.path().join("text_test_file.txt");

        let mut file = File::create(&text_file_path).expect("Unable to create file.");
        file.write_all(b"Hello, world!")
            .expect("Unable to write to file.");

        let result = chunk_file(&text_file_path, 0);
        assert_eq!(result.0, "Hello, world!");
    }

    #[test]
    fn test_binary_content_check() {
        let dir = tempdir().expect("Unable to create temporary directory.");
        let file_path = dir.path().join("binary.txt");

        let mut file = File::create(&file_path).expect("Unable to create file.");
        file.write_all(&[0u8, 1, 2, 3, 4, 255, 0b10000000])
            .expect("Unable to write to file.");

        let result = chunk_file(&file_path, 0);
        assert_eq!(result.0, "The provided file appears to be binary and cannot be processed.");
    }
}
