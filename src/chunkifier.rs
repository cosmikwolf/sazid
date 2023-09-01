use tiktoken_rs::cl100k_base;
use crate::pdf_extractor::PdfText;
use crate::session_manager::INGESTED_DIR;
use std::fs::{self, File};
use std::io::Read;
use std::path::{PathBuf, Path};
use std::str::FromStr;
use crate::errors::ChunkifierError;
pub struct Chunkifier;

impl Chunkifier {
    /// This function determines if the input is a file path or just a block of text.
    /// If the input is a valid file path, it will chunkify the contents of the file.
    /// Otherwise, it will treat the input as plain text and chunkify it directly.
    pub fn chunkify_input(input: &str, tokens_per_chunk: usize) -> Result<Vec<String>, ChunkifierError> {
        let path = PathBuf::from_str(input);
        // Check if the input can be treated as a file path
        if let Ok(p) = path {
            if p.is_file() {
                // If it's a file, chunkify its contents
                return Self::chunkify_file(&p, tokens_per_chunk);
            } else if p.is_dir() {
                let dirchunks = p.read_dir()
                    .map_err(|_| ChunkifierError::Other("Failed to read directory".to_string()))
                    .and_then(|dir| {
                        dir.map(|entry| entry.map(|e| e.path()))
                            .collect::<Result<Vec<_>, std::io::Error>>()
                            .map_err(|_| {
                                ChunkifierError::Other("Failed to read directory".to_string())
                            })
                    })
                    .and_then(|paths| {
                        paths
                            .iter()
                            .map(|path| Self::chunkify_file(path, tokens_per_chunk))
                            .collect::<Result<Vec<_>, ChunkifierError>>()
                    })
                    .map(|chunks| chunks.into_iter().flatten().collect()).unwrap();
                return Ok(dirchunks);
            } else {
                // if it is a path, but its not a file or a directory, then it is a URL
                println!("URL detected, but not implemented. ingesting as text");
                return Ok::<Vec<std::string::String>, ChunkifierError>(Self::chunkify_text(input, tokens_per_chunk));
            }
        } else {
            // If not a file path, chunkify the input text directly
            return Ok::<Vec<std::string::String>, ChunkifierError>(Self::chunkify_text(input, tokens_per_chunk));
        };
    }

    /// Ingest a file by chunkifying its contents  


    /// Chunk the content of a file based on its type (PDF, text, etc.).
    /// Additionally, copy the file to the 'ingested' directory after chunking.
    fn chunkify_file(
        file_path: &PathBuf,
        tokens_per_chunk: usize,
    ) -> Result<Vec<String>, ChunkifierError> {
        let content = Self::extract_file_text(file_path)?;
        let chunks = Self::chunkify_text(&content, tokens_per_chunk);
        if file_path.is_file() {
            let dest_path = 
            Path::new(INGESTED_DIR)
                .join("ingested")
                .join(file_path.file_name().unwrap());
            fs::copy(&file_path, &dest_path)?;
        }
        Ok(chunks)
    }
    
    fn chunkify_text(text: &str, tokens_per_chunk: usize) -> Vec<String> {
        let tokens: Vec<&str> = text.split_whitespace().collect();
        let bpe = cl100k_base().unwrap();
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

    fn extract_file_text(file_path: &PathBuf) -> Result<String, ChunkifierError> {
        if Self::is_pdf_file(file_path) {
            PdfText::from_pdf(file_path)
                .and_then(|pdf_text| pdf_text.get_text())
                .map_err(|_| {
                    ChunkifierError::Other("Failed to extract text from PDF".to_string())
                })
        } else if Self::is_binary_file(file_path) {
            Err(ChunkifierError::Other(
                "Binary file detected".to_string(),
            ))
        } else {
            fs::read_to_string(file_path).map_err(|_| {
                ChunkifierError::Other("Failed to read text file".to_string())
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
            let chunks = Chunkifier::chunkify_file(&pdf_file_path, 4).unwrap();

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
            let text_file_path = dir.path().join("test.txt");

            File::create(&text_file_path)
                .unwrap()
                .write_all(b"Hello, world!\nHow are you?\nThis is a test!")
                .unwrap();

            let chunks = Chunkifier::chunkify_file(&text_file_path, 4).unwrap();

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
            let dir = tempdir().unwrap();
            let binary_file_path = dir.path().join("binary_test_file.bin");

            File::create(&binary_file_path)
                .unwrap()
                .write_all(&[0u8, 1, 2, 3, 4, 255])
                .unwrap();

            let result = Chunkifier::chunkify_file(&binary_file_path, 4);

            // We expect an error as the binary file is not processable.
            assert!(result.is_err());
        }
    }
}
