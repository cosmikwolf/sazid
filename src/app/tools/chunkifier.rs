use crate::errors::ChunkifierError;
use crate::consts::*;
use crate::types::*;
use crate::types::PdfText;
use crate::utils;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use tiktoken_rs::cl100k_base;

    // takes input text and returns chunks with all data extracted
    pub fn parse_input(
        input: &str,
        tokens_per_chunk: usize,
        model_max_tokens: usize,
    ) -> Result<Vec<String>, ChunkifierError> {
        let ingest_data = categorize_input(input)?;
        let chunks = chunkify_parsed_input(ingest_data, tokens_per_chunk).unwrap();
        check_token_count_model_limit(&chunks, model_max_tokens).unwrap();
        Ok(chunks)
    }

    fn categorize_input(input: &str) -> Result<IngestData, ChunkifierError> {
        let ingest_data = IngestData {
            text: input.to_string(),
            urls: Vec::new(),
            file_paths: Vec::new(),
        };
        return Ok(ingest_data);
        // this code has a bug where any text that shared the name of a local directory results in a directories not supported error. need to just make this an argument -f
        // let words: Vec<&str> = input.split_whitespace().collect();
        // for word in words {
        //     if let Ok(url) = url::Url::parse(word) {
        //         ingest_data.urls.push(url.to_string());
        //         continue;
        //     } else {
        //         let path = PathBuf::try_from(word);
        //         if let Ok(p) = path {
        //             if p.exists() && p.is_file() {
        //                 ingest_data.file_paths.push(p);
        //             } else if p.exists() && p.is_dir() {
        //                 return Err(ChunkifierError::Other(format!("Directories are not supported. path: {:?}", p)));
        //             } else {
        //                 // its not a file or a directory, so it must be text
        //             }
        //         }
        //     }
        // }
        // Ok(ingest_data)
    }

    fn chunkify_parsed_input(
        ingest_data: IngestData,
        tokens_per_chunk: usize,
    ) -> Result<Vec<String>, ChunkifierError> {
        let full_text = ingest_data.text;
        // ingest_data.urls.iter().for_each(|url| {
        //     let url_data = UrlData {
        //         urls: url.to_string(),
        //         data: reqwest::blocking::get(url).unwrap().text().unwrap()
        //     };
        //     full_text.push_str(&url_data.data);
        // });
        // ingest_data.file_paths.iter().for_each(|path| {
        //     full_text.push_str(&Self::extract_file_text(path).unwrap());
        // });
        Ok(chunkify_text(&full_text, tokens_per_chunk))
    }

    /// an algorithm that will determine if a Vec<String> string exceeds a model_max_tokens limit
    pub fn check_token_count_model_limit(
        chunks: &Vec<String>,
        model_max_tokens: usize,
    ) -> Result<(), ChunkifierError> {
        let bpe = cl100k_base().unwrap();
        let mut token_count = 0;
        for chunk in chunks {
            token_count += bpe.encode_with_special_tokens(chunk).len();
        }
        if token_count > model_max_tokens {
            Err(ChunkifierError::Other(format!(
                "Input exceeds max token limit: {} tokens",
                token_count
            )))
        } else {
            Ok(())
        }
    }

    // write a function that determines if any words in a string are a URL, file path, or text
    //

    /// This function determines if the input is a file path or just a block of text.
    /// If the input is a valid file path, it will chunkify the contents of the file.
    /// Otherwise, it will treat the input as plain text and chunkify it directly.
    pub fn chunkify_input(
        input: &str,
        tokens_per_chunk: usize,
    ) -> Result<Vec<String>, ChunkifierError> {
        let path = PathBuf::try_from(input);
        // Check if the input can be treated as a file path

        if let Ok(p) = path {
            if p.is_file() {
                // If it's a file, chunkify its contents
                chunkify_file(&p, tokens_per_chunk)
            } else if p.is_dir() {
                let dirchunks = p
                    .read_dir()
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
                            .map(|path| chunkify_file(path, tokens_per_chunk))
                            .collect::<Result<Vec<_>, ChunkifierError>>()
                    })
                    .map(|chunks| chunks.into_iter().flatten().collect())
                    .unwrap();
                return Ok(dirchunks);
            } else {
                // if it is a path, but its not a file or a directory, then it is a URL
                println!("URL detected, but not implemented. ingesting as text");
                return Ok::<Vec<std::string::String>, ChunkifierError>(chunkify_text(
                    input,
                    tokens_per_chunk,
                ));
            }
        } else {
            // If not a file path, chunkify the input text directly
            Ok::<Vec<std::string::String>, ChunkifierError>(chunkify_text(
                input,
                tokens_per_chunk,
            ))
        }
    }

    /// Ingest a file by chunkifying its contents  

    /// Chunk the content of a file based on its type (PDF, text, etc.).
    /// Additionally, copy the file to the 'ingested' directory after chunking.
    fn chunkify_file(
        file_path: &PathBuf,
        tokens_per_chunk: usize,
    ) -> Result<Vec<String>, ChunkifierError> {
        let content = extract_file_text(file_path)?;
        let chunks = chunkify_text(&content, tokens_per_chunk);
        utils::ensure_directory_exists(INGESTED_DIR).unwrap();
        if file_path.is_file() {
            let dest_path = Path::new(INGESTED_DIR).join(file_path.file_name().unwrap());
            fs::copy(file_path, dest_path)?;
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
        if is_pdf_file(file_path) {
            PdfText::from_pdf(file_path)
                .and_then(|pdf_text| pdf_text.get_text())
                .map_err(|_| ChunkifierError::Other("Failed to extract text from PDF".to_string()))
        } else if is_binary_file(file_path) {
            Err(ChunkifierError::Other("Binary file detected".to_string()))
        } else {
            fs::read_to_string(file_path)
                .map_err(|_| ChunkifierError::Other("Failed to read text file".to_string()))
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

        // a test for parse_input to verify that it is doing what it needs to do
        // using fake text that has a URL, a filepath and also some text
        // it should return a IngestData struct that contains the full text, a list of URLs, and a list of file paths
        #[test]
        fn test_parse_input() {
            let input = "https://www.google.com/ src/main.rs this is some text";
            let ingest_data = categorize_input(input).unwrap();
            assert_eq!(
                ingest_data.text,
                "https://www.google.com/ src/main.rs this is some text"
            );
            assert_eq!(ingest_data.urls, vec!["https://www.google.com/"]);
            assert_eq!(ingest_data.file_paths, vec![PathBuf::from("src/main.rs")]);
        }

        #[test]
        fn test_chunkify_pdf_file() {
            let pdf_file_path = PathBuf::from("tests/data/NIST.SP.800-185.pdf");
            let chunks = chunkify_file(&pdf_file_path, 4).unwrap();

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

            let chunks = chunkify_file(&text_file_path, 4).unwrap();

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

            let result = chunkify_file(&binary_file_path, 4);

            // We expect an error as the binary file is not processable.
            assert!(result.is_err());
        }
    }
}
