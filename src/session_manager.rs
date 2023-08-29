use crate::file_chunker::FileChunker;
use crate::file_chunker::FileChunkerError;  
use crate::gpt_connector::{ChatCompletionRequestMessage, GPTConnector};
use chrono::Local;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;
use serde_json;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use uuid::Uuid;

pub struct SessionManager {
    session_id: String,
    tokens_per_chunk: usize,
    base_dir: PathBuf,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct IngestedData {
    session_id: String,
    file_path: String,
    chunk_num: u32,
    content: String,
}
impl SessionManager {
    // Create a new SessionManager with a specified base directory.
    pub fn new(base_dir: PathBuf) -> Self {
        SessionManager {
            base_dir,
            session_id: Uuid::new_v4().to_string(),
            tokens_per_chunk: 4, // or whatever default chunk size you prefer
        }
    }

    // Ensure the session_data directory exists.
    fn ensure_session_data_directory_exists(&self) {
        let path = self.base_dir.join("session_data");
        if !path.exists() {
            fs::create_dir(&path).expect("Failed to create session_data directory");
        }
    }

    // Generate a new session filename based on the current date, time, and a random 16-bit hash.
    pub fn new_session_filename(&self) -> String {
        let current_time = Local::now().format("%Y%m%d%H%M").to_string();
        let random_hash: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .map(|b| b as char)
            .take(4)
            .collect();
        let filename = format!("{}_{}.json", current_time, random_hash);
        filename
    }

    // Load a session from a given filename.
    pub fn load_session(
        &self,
        filename: &str,
    ) -> Result<Vec<ChatCompletionRequestMessage>, std::io::Error> {
        self.ensure_session_data_directory_exists();
        let data = fs::read(self.base_dir.join("session_data").join(filename))?;
        let messages = serde_json::from_slice(&data).unwrap_or_default();
        Ok(messages)
    }

    // Save a session to a given filename.
    pub fn save_session(
        &self,
        filename: &str,
        messages: &Vec<ChatCompletionRequestMessage>,
    ) -> Result<(), std::io::Error> {
        self.ensure_session_data_directory_exists();
        let data = serde_json::to_vec(messages)?;
        fs::write(self.base_dir.join("session_data").join(filename), data)?;
        Ok(())
    }

    // Load the last used session filename.
    pub fn load_last_session_filename(&self) -> Option<String> {
        self.ensure_session_data_directory_exists();
        if let Ok(filename) =
            fs::read_to_string(self.base_dir.join("session_data/last_session.txt"))
        {
            return Some(filename);
        }
        None
    }

    // Save the last used session filename.
    pub fn save_last_session_filename(&self, filename: &str) -> Result<(), std::io::Error> {
        self.ensure_session_data_directory_exists();
        fs::write(
            self.base_dir.join("session_data/last_session.txt"),
            filename,
        )?;
        Ok(())
    }

    // Delete a session.
    pub fn delete_session(&self, filename: &str) -> Result<(), std::io::Error> {
        self.ensure_session_data_directory_exists();
        let path = self.base_dir.join("session_data").join(filename);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn save_ingested_data_log(
        &self,
        filename: &str,
        data: &str,
        chunk_num: usize,
        token_count: usize,
    ) -> Result<(), std::io::Error> {
        let log_path = self.base_dir.join("session_data/ingested");
        if !log_path.exists() {
            fs::create_dir_all(&log_path)?;
        }

        let log_file = format!("{}_ingest.json", filename);
        let log_content = serde_json::json!({
            "file_path": data,
            "chunk_num": chunk_num,
            "timestamp": Local::now().to_string(),
            "tokens_used": token_count
        });

        fs::write(log_path.join(log_file), log_content.to_string())?;
        Ok(())
    }

    // Copy ingested file to its new directory.
    pub fn copy_ingested_file(
        &self,
        src_path: &PathBuf,
        filename: &str,
    ) -> Result<(), std::io::Error> {
        let dest_dir = self
            .base_dir
            .join(format!("session_data/ingested/{}_files", filename));
        if !dest_dir.exists() {
            fs::create_dir_all(&dest_dir)?;
        }

        let dest_path = dest_dir.join(src_path.file_name().unwrap());
        fs::copy(src_path, dest_path)?;
        Ok(())
    }

    /// This function takes in an input which could be a path to a directory, a path to a file,
    /// a block of text, or a URL. Depending on the type of input, it processes (or ingests) the 
    /// content by converting it into chunks of text and then sends each chunk to the GPT API.
    pub fn handle_ingest(&self, input: &str) -> Result<(), FileChunkerError> {
        let mut gpt_connector = GPTConnector::new();
        
        // This vector will store paths that need to be processed.
        let mut paths_to_process = Vec::new();

        // Try to interpret the input as a path.
        let input_path = PathBuf::from_str(input);
        
        // If it's a valid path, check if it points to a directory or a file.
        if let Ok(p) = input_path {
            if p.is_dir() {
                // If it's a directory, iterate through its contents and add all the file paths to the processing list.
                for entry in fs::read_dir(&p)? {
                    let entry_path = entry?.path();
                    if entry_path.is_file() {
                        paths_to_process.push(entry_path);
                    }
                }
            } else if p.is_file() {
                // If it's a file, add it directly to the processing list.
                paths_to_process.push(p);
            }
        }

        // If the list is empty, assume the input is a block of text and treat it accordingly.
        if paths_to_process.is_empty() {
            paths_to_process.push(PathBuf::from(input));
        }

        // Iterate through all the paths to process them.
        for path in paths_to_process {
            let chunks = if path.is_file() {
                // If it's a file, chunkify its contents.
                FileChunker::chunkify_input(path.to_str().unwrap(), self.tokens_per_chunk)?
            } else {
                // Otherwise, chunkify the input directly.
                FileChunker::chunkify_input(input, self.tokens_per_chunk)?
            };

            // Send each chunk to the GPT API using the GPTConnector.
            for chunk in chunks {
                gpt_connector.send(&chunk)?;
            }

            // After successful ingestion, copy the file to the 'ingested' directory.
            if path.is_file() {
                let dest_path = self.base_dir.join("ingested").join(path.file_name().unwrap());
                fs::copy(&path, &dest_path)?;
            }
        }

        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpt_connector::Role;
    use std::fs::{self, File};
    use std::io::Write;
    use tempdir::TempDir;
    use tempfile::tempdir;

    #[test]
    fn test_save_ingested_data_log() {
        let dir = tempdir().unwrap();
        let manager = SessionManager::new(dir.path().to_path_buf());
        let filename = "test_session";
        manager
            .save_ingested_data_log(filename, "test_data", 1, 500)
            .unwrap();

        // Verify the file exists and has the expected content
        let log_path = dir
            .path()
            .join("session_data/ingested/test_session_ingest.json");
        assert!(log_path.exists());
        let content = fs::read_to_string(log_path).unwrap();
        assert!(content.contains("test_data"));
        assert!(content.contains("\"chunk_num\":1"));
        assert!(content.contains("\"tokens_used\":500"));
    }

    #[test]
    fn test_copy_ingested_file() {
        let dir = tempdir().unwrap();
        let manager = SessionManager::new(dir.path().to_path_buf());

        let src_path = dir.path().join("source.txt");
        File::create(&src_path)
            .unwrap()
            .write_all(b"Hello, World!")
            .unwrap();

        manager
            .copy_ingested_file(&src_path, "test_session")
            .unwrap();

        let dest_path = dir
            .path()
            .join("session_data/ingested/test_session_files/source.txt");
        assert!(dest_path.exists());
        let content = fs::read_to_string(dest_path).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_handle_ingest_plain_text() {
        // Setup
        let temp_dir = TempDir::new("test_directory").unwrap();
        let base_dir = temp_dir.path().to_path_buf();

        let session_manager = SessionManager::new(base_dir.clone());
        let file_path = PathBuf::from("tests/data/testText1.txt"); // Adjust the path

        // Call the function
        let chunks = session_manager
            .handle_ingest(&file_path)
            .unwrap();

        // Verify chunking
        assert!(!chunks.is_empty(), "No chunks created for plain text file");

        // Verify file storage
        let dest_path = base_dir.join("ingested/test_session_files/path_to_sample_text_file.txt");
        assert!(
            dest_path.exists(),
            "Ingested file not found in designated directory"
        );

        // Verify log files
        for i in 0..chunks.len() {
            let log_file_name = format!("{}_ingest.json", i + 1);
            let log_path = base_dir.join("ingested").join(log_file_name);
            assert!(log_path.exists(), "Log file for chunk {} not found", i + 1);
        }
    }

    #[test]
    fn test_handle_ingest_pdf() {
        // Setup
        let temp_dir = TempDir::new("test_directory").unwrap();
        let base_dir = temp_dir.path().to_path_buf();

        let session_manager = SessionManager::new(base_dir.clone());
        let file_path = PathBuf::from("tests/data/NIST.SP.800-185.pdf"); // Adjust the path

        // Call the function
        let chunks = session_manager
            .handle_ingest(&file_path)
            .unwrap();

        // Verify chunking
        assert!(!chunks.is_empty(), "No chunks created for PDF file");

        // Verify file storage
        let dest_path = base_dir.join("ingested/test_session_files/path_to_sample_pdf_file.pdf");
        assert!(
            dest_path.exists(),
            "Ingested file not found in designated directory"
        );

        // Verify log files
        for i in 0..chunks.len() {
            let log_file_name = format!("{}_ingest.json", i + 1);
            let log_path = base_dir.join("ingested").join(log_file_name);
            assert!(log_path.exists(), "Log file for chunk {} not found", i + 1);
        }
    }
    #[test]
    fn test_session_management() {
        let manager = SessionManager::new(PathBuf::from("./"));

        // Test session filename generation
        let filename = manager.new_session_filename();
        assert!(filename.contains("_"));

        // Test session saving and loading
        let messages = vec![ChatCompletionRequestMessage {
            role: Role::User,
            content: "Test message".to_string(),
        }];
        manager.save_session(&filename, &messages).unwrap();
        let loaded_messages = manager.load_session(&filename).unwrap();
        assert_eq!(messages, loaded_messages);

        // Test last session filename saving and loading
        manager.save_last_session_filename(&filename).unwrap();
        let last_session_filename = manager.load_last_session_filename().unwrap();
        assert_eq!(filename, last_session_filename);
    }
}
