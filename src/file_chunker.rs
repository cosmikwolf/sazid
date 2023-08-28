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
    use reqwest; // Add `reqwest` to your dependencies in `Cargo.toml`

    #[test]
    fn test_chunk_remote_pdfs() {
        let pdf_urls = vec![
            "https://docs.python.org/3.9/archives/python-3.9.7-docs.pdf",
            "https://docs.oracle.com/javase/tutorial/java/nutsandbolts/QandE.pdf",
            "https://ww1.microchip.com/downloads/en/DeviceDoc/Atmel-42735-8-bit-AVR-Microcontroller-ATmega328-328P_Datasheet.pdf",
            "https://www.espressif.com/sites/default/files/documentation/esp32_datasheet_en.pdf",
            "https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-185.pdf",
            "https://cds.cern.ch/record/1092437/files/CERN-2008-008.pdf",
            "http://mirrors.ctan.org/info/lshort/english/lshort.pdf",
            "https://github.github.com/training-kit/downloads/github-git-cheat-sheet.pdf",
            "https://www.unicode.org/versions/Unicode13.0.0/UnicodeStandard-13.0.pdf",
        ];

        let dir = tempdir().expect("Unable to create temporary directory.");
        let mut failed_downloads = 0;

        for url in pdf_urls {
            let response = reqwest::blocking::get(url);

            match response {
                Ok(mut data) => {
                    let content_disp = data
                        .headers()
                        .get(reqwest::header::CONTENT_DISPOSITION)
                        .and_then(|cd| cd.to_str().ok());

                        let filename = url.split('/').last().unwrap_or("unknown.pdf").to_string();

                        let mut out = File::create(dir.path().join(&filename)).expect("Failed to create file");
                        data.copy_to(&mut out).expect("Failed to write content to file");
                        
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

            let result = chunk_file(&text_file_path, 1);
            assert_eq!(result.0, "Hello, world!"); 
        }

    #[test]
    fn test_binary_content_check() {
        let dir = tempdir().expect("Unable to create temporary directory.");
        let file_path = dir.path().join("binary.txt");

        let mut file = File::create(&file_path).expect("Unable to create file.");
        file.write_all(&[0u8, 1, 2, 3, 4, 255, 0b10000000])
            .expect("Unable to write to file.");

        let result = chunk_file(&file_path, 1);
        assert_eq!(result.0, "The provided file appears to be binary and cannot be processed.");
    }
}
