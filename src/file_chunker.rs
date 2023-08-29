use crate::pdf_extractor::PdfText;
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;

pub struct FileChunker;

#[derive(Debug)]
pub enum FileChunkerError {
    ChunkingError(String),
    FileReadError,
    UnsupportedFileType, // Any other specific errors can be added here in the future
}
impl fmt::Display for FileChunkerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FileChunkerError::UnsupportedFileType => write!(f, "Unsupported file type"),
            FileChunkerError::FileReadError => write!(f, "Error reading the file"),
            FileChunkerError::ChunkingError(description) => write!(f, "{}", description),
            // ... handle other variants similarly ...
        }
    }
}
impl std::error::Error for FileChunkerError {}

impl FileChunker {
    /// Chunk the content of a file based on its type (PDF, text, etc.).
    pub fn chunkify_file(
        file_path: &PathBuf,
        tokens_per_chunk: usize,
    ) -> Result<Vec<String>, FileChunkerError> {
        let content = Self::extract_file_text(file_path)?;
        let chunks = Self::chunk_content(&content, tokens_per_chunk);
        Ok(chunks)
    }
    pub fn extract_file_text(file_path: &PathBuf) -> Result<String, FileChunkerError> {
        let content = if Self::is_binary_file(&file_path) {
            if Self::is_pdf_file(&file_path) {
                let pdf_text = PdfText::from_pdf(&file_path).map_err(|_| {
                    FileChunkerError::ChunkingError("Failed to extract text from PDF".to_string())
                })?;
                (1usize..=pdf_text.total_pages())
                    .filter_map(|page| pdf_text.get_page_text(page as u32))
                    .flatten()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
                    .join("\n")
            } else {
                return Err(FileChunkerError::ChunkingError(
                    "The provided file appears to be binary and cannot be processed.".to_string(),
                ));
            }
        } else {
            fs::read_to_string(file_path).map_err(|_| {
                FileChunkerError::ChunkingError("Failed to read text file".to_string())
            })?
        };

        Ok(content)
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

    pub fn chunk_content(content: &str, chunk_size: usize) -> Vec<String> {
        content
            .chars()
            .collect::<Vec<_>>()
            .chunks(chunk_size)
            .map(|chunk| chunk.iter().collect())
            .collect()
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
            let chunks = FileChunker::chunkify_file(&pdf_file_path, 4).unwrap();

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
            let dir = tempdir().unwrap();
            let text_file_path = dir.path().join("text_test_file.txt");

            File::create(&text_file_path)
                .unwrap()
                .write_all(b"Hello, world!\nHow are you?\nThis is a test!")
                .unwrap();

            let chunks = FileChunker::chunkify_file(&text_file_path, 4).unwrap();

            assert_eq!(chunks.len(), 3);
            assert_eq!(chunks[0], "Hello, world!");
            assert_eq!(chunks[1], "How are you?");
            assert_eq!(chunks[2], "This is a test!");
        }

        #[test]
        fn test_chunkify_binary_file() {
            let dir = tempdir().unwrap();
            let binary_file_path = dir.path().join("binary_test_file.bin");

            File::create(&binary_file_path)
                .unwrap()
                .write_all(&[0u8, 1, 2, 3, 4, 255])
                .unwrap();

            let result = FileChunker::chunkify_file(&binary_file_path, 4);

            // We expect an error as the binary file is not processable.
            assert!(result.is_err());
        }
    }
}
