use crate::pdf_extractor::PdfText;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

pub struct FileChunker;

impl FileChunker {
    /// Chunk the given file based on its type (PDF, text, etc.).
    pub fn chunk_file(file_path: &PathBuf, index: usize) -> (String, usize) {
        if Self::is_binary_file(&file_path) {
            if Self::is_pdf_file(&file_path) {
                return Self::chunk_pdf_file(file_path, index);
            } else {
                return (
                    String::from("The provided file appears to be binary and cannot be processed."),
                    0,
                );
            }
        }

        let ext = file_path.extension().and_then(|s| s.to_str());
        match ext {
            Some("pdf") => Self::chunk_pdf_file(file_path, index),
            _ => Self::chunk_text_file(file_path, index), // Default to text file chunking
        }
    }

    /// Check if the given file is a PDF.
    fn is_pdf_file(file_path: &PathBuf) -> bool {
        file_path.extension().and_then(|s| s.to_str()) == Some("pdf")
    }

    /// Check if the given file appears to be a binary file.
    fn is_binary_file(file_path: &PathBuf) -> bool {
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
    fn chunk_pdf_file(file_path: &PathBuf, index: usize) -> (String, usize) {
        let pdf_text = PdfText::from_pdf(file_path).expect("Failed to extract text from PDF.");
        let total_pages = pdf_text.total_pages();

        // Print errors for debugging
        if !pdf_text.errors.is_empty() {
            for error in &pdf_text.errors {
                println!("PDF Extraction Error: {}", error);
            }
        } else {
            println!("No errors encountered while extracting PDF.");
        }

        println!("Total pages: {}", total_pages);
        println!("Requested page: {}", index);
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
    fn chunk_text_file(file_path: &PathBuf, index: usize) -> (String, usize) {
        let file = File::open(file_path).expect("Failed to open file.");
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

        if index == 0 || index > lines.len() {
            (String::from("Index out of bounds."), lines.len())
        } else {
            (lines[index - 1].clone(), lines.len())
        }
    }

    pub fn chunk_content(content: &str, chunk_size: usize) -> Vec<String> {
        content.chars().collect::<Vec<_>>().chunks(chunk_size).map(|chunk| chunk.iter().collect()).collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use reqwest;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir; // Add `reqwest` to your dependencies in `Cargo.toml`

    #[test]
    fn test_chunk_local_pdfs() {
        use std::fs;

        // List all files in the tests/data directory
        let paths = fs::read_dir("tests/data").expect("Failed to read directory");
        let mut pdf_count = 0;

        for path in paths {
            let path = path.expect("Failed to read path").path();

            // Ensure the file is a PDF
            if path.extension().and_then(|s| s.to_str()) == Some("pdf") {
                println!("Processing: {:?}", path);
                let (chunk, total_pages) = FileChunker::chunk_pdf_file(&path, 1);
                assert_ne!(chunk, String::from("Index out of bounds."));
                assert!(total_pages > 0);
                pdf_count += 1;
            }
        }

        assert!(
            pdf_count >= 2,
            "There should be at least 2 PDFs in the tests/data directory"
        );
    }

    #[test]
    fn test_chunk_remote_pdfs() {
        let pdf_urls = vec![
            "https://docs.python.org/3.9/archives/python-3.9.7-docs.pdf",
            "https://docs.oracle.com/javase/tutorial/java/nutsandbolts/QandE.pdf",
            "https://ww1.microchip.com/downloads/en/DeviceDoc/Atmel-42735-8-bit-AVR-Microcontroller-ATmega328-328P_Datasheet.pdf",
            "https://cds.cern.ch/record/1092437/files/CERN-2008-008.pdf",
        ];

        let dir = tempdir().expect("Unable to create temporary directory.");
        let mut failed_downloads = 0;

        for url in pdf_urls {
            let response = reqwest::blocking::get(url);

            match response {
                Ok(mut data) => {
                    let _content_disp = data
                        .headers()
                        .get(reqwest::header::CONTENT_DISPOSITION)
                        .and_then(|cd| cd.to_str().ok());

                    let filename = url.split('/').last().unwrap_or("unknown.pdf").to_string();

                    let mut out =
                        File::create(dir.path().join(&filename)).expect("Failed to create file");
                    data.copy_to(&mut out)
                        .expect("Failed to write content to file");
                }
                Err(e) => {
                    println!(
                        "Warning: Failed to download PDF from {}. If you see more than 2 of these warnings, consider updating the test URLs. Error: {}",
                        url, e
                    );
                    failed_downloads += 1;
                }
            }
        }

        assert!(
            failed_downloads <= 2,
            "Failed to download more than two PDFs. Consider updating the test URLs."
        );
    }

    #[test]
    fn test_chunk_binary_file() {
        let dir = tempdir().expect("Unable to create temporary directory.");
        let binary_file_path = dir.path().join("binary_test_file.bin");

        let mut file = File::create(&binary_file_path).expect("Unable to create file.");
        file.write_all(&[0u8, 1, 2, 3, 4, 255])
            .expect("Unable to write to file.");

        let result = FileChunker::chunk_file(&binary_file_path, 0);
        assert_eq!(
            result.0,
            "The provided file appears to be binary and cannot be processed."
        );
    }

    #[test]
    fn test_chunk_text_file() {
        let dir = tempdir().expect("Unable to create temporary directory.");
        let text_file_path = dir.path().join("text_test_file.txt");

        let mut file = File::create(&text_file_path).expect("Unable to create file.");
        file.write_all(b"Hello, world!")
            .expect("Unable to write to file.");

        let result = FileChunker::chunk_file(&text_file_path, 1);
        assert_eq!(result.0, "Hello, world!");
    }

    #[test]
    fn test_binary_content_check() {
        let dir = tempdir().expect("Unable to create temporary directory.");
        let file_path = dir.path().join("binary.txt");

        let mut file = File::create(&file_path).expect("Unable to create file.");
        file.write_all(&[0u8, 1, 2, 3, 4, 255, 0b10000000])
            .expect("Unable to write to file.");

        let result = FileChunker::chunk_file(&file_path, 1);
        assert_eq!(
            result.0,
            "The provided file appears to be binary and cannot be processed."
        );
    }
}
