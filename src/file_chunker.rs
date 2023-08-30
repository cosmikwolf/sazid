use tiktoken_rs::p50k_base;
use crate::pdf_extractor::PdfText;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use crate::errors::FileChunkerError;
pub struct FileChunker;

impl FileChunker {
    /// This function determines if the input is a file path or just a block of text.
    /// If the input is a valid file path, it will chunkify the contents of the file.
    /// Otherwise, it will treat the input as plain text and chunkify it directly.
    pub fn chunkify_input(input: &str, tokens_per_chunk: usize) -> Result<Vec<String>, FileChunkerError> {
        let path = PathBuf::from_str(input);
        
        // Check if the input can be treated as a file path
        if let Ok(p) = path {
            if p.is_file() {
                // If it's a file, chunkify its contents
                return Self::chunkify_file(&p, tokens_per_chunk);
            }
        }

        // If not a file path, chunkify the input text directly
        Ok(Self::chunkify_text(input, tokens_per_chunk))
    }

    /// Chunk the content of a file based on its type (PDF, text, etc.).
    fn chunkify_file(
        file_path: &PathBuf,
        tokens_per_chunk: usize,
    ) -> Result<Vec<String>, FileChunkerError> {
        let content = Self::extract_file_text(file_path)?;
        let chunks = Self::chunkify_text(&content, tokens_per_chunk);
        Ok(chunks)
    }
    
    fn chunkify_text(text: &str, tokens_per_chunk: usize) -> Vec<String> {
        let tokens: Vec<&str> = text.split_whitespace().collect();
        let bpe = p50k_base().expect("Expected successful operation");
        let mut chunks = Vec::new();
        let mut current_chunk = Vec::new();
        let mut current_token_count = 0;

        for token in tokens {
            let new_token_count = bpe.encode_with_special_tokens(token).len();

            if current_token_count + new_token_count > tokens_per_chunk {
                chunks.push(current_chunk.join(" "));
                current_chunk.clear();
                current_token_count = 0;
            }

            current_token_count += new_token_count;
            current_chunk.push(token);
        }

        if !current_chunk.is_empty() {
            chunks.push(current_chunk.join(" "));
        }

        chunks
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

    fn extract_file_text(file_path: &PathBuf) -> Result<String, FileChunkerError> {
        if Self::is_pdf_file(file_path) {
            PdfText::from_pdf(file_path)
                .and_then(|pdf_text| pdf_text.get_text())
                .map_err(|_| {
                    FileChunkerError::Other("Failed to extract text from PDF".to_string())
                })
        } else if Self::is_binary_file(file_path) {
            Err(FileChunkerError::Other(
                "Binary file detected".to_string(),
            ))
        } else {
            fs::read_to_string(file_path).map_err(|_| {
                FileChunkerError::Other("Failed to read text file".to_string())
            })
        }
    }

}
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::fs::File;
        use std::io::Write;
        use tempfile::tempdir;

        #[test]
        fn test_chunkify_pdf_file() {
            let pdf_file_path = PathBuf::from("tests/data/NIST.SP.800-185.pdf");
            let chunks = FileChunker::chunkify_file(&pdf_file_path, 4).expect("Expected successful operation");

            // This will depend on the content of the PDF and the chunk size.
            // For the purpose of the test, let's check if the first chunk contains some expected stub text.
            // You can adjust the expected stub text based on the content of the PDF.
            let expected_text = "NIST Special Publication"; // Adjust this as necessary

            assert!(!chunks.is_empty());
            assert!(
                chunks[0].contains(expected_text),
                "Expected stub text not found in the first chunk."
            );

            // Print out the chunks for verification:
            for (i, chunk) in chunks.iter().enumerate() {
                println!("Chunk {}: {}", i + 1, chunk);
            }
        }

        #[test]
        fn test_chunkify_text_file() {
            let dir = tempdir().expect("Expected successful operation");
            let text_file_path = dir.path().join("test.txt");

            File::create(&text_file_path)
                .expect("Expected successful operation")
                .write_all(b"Hello, world!\nHow are you?\nThis is a test!")
                .expect("Expected successful operation");

            let chunks = FileChunker::chunkify_file(&text_file_path, 4).expect("Expected successful operation");

            // Print out the chunks for verification:
            for (i, chunk) in chunks.iter().enumerate() {
                println!("Chunk {}: {}", i + 1, chunk);
            }
            assert_eq!(chunks.len(), 4);
            assert_eq!(chunks[0], "Hello, world!");
            assert_eq!(chunks[1], "How are you?");
            assert_eq!(chunks[2], "This is a");
            assert_eq!(chunks[3], "test!");
        }

        #[test]
        fn test_chunkify_binary_file() {
            let dir = tempdir().expect("Expected successful operation");
            let binary_file_path = dir.path().join("binary_test_file.bin");

            File::create(&binary_file_path)
                .expect("Expected successful operation")
                .write_all(&[0u8, 1, 2, 3, 4, 255])
                .expect("Expected successful operation");

            let result = FileChunker::chunkify_file(&binary_file_path, 4);

            // We expect an error as the binary file is not processable.
            assert!(result.is_err());
        }
    }
}
