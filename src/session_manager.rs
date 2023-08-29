use crate::gpt_connector::ChatCompletionRequestMessage;
use crate::gpt_connector::GPTConnector;
use crate::gpt_connector::Role;
use crate::file_chunker;
use chrono::Local;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde_json;
use std::fs;
use std::path::PathBuf;

pub struct SessionManager {
    base_dir: PathBuf,
}

impl SessionManager {
    // Create a new SessionManager with a specified base directory.
    pub fn new(base_dir: PathBuf) -> Self {
        SessionManager { base_dir }
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
    pub fn load_session(&self, filename: &str) -> Result<Vec<ChatCompletionRequestMessage>, std::io::Error> {
        self.ensure_session_data_directory_exists();
        let data = fs::read(self.base_dir.join("session_data").join(filename))?;
        let messages = serde_json::from_slice(&data).unwrap_or_default();
        Ok(messages)
    }

    // Save a session to a given filename.
    pub fn save_session(&self, filename: &str, messages: &Vec<ChatCompletionRequestMessage>) -> Result<(), std::io::Error> {
        self.ensure_session_data_directory_exists();
        let data = serde_json::to_vec(messages)?;
        fs::write(self.base_dir.join("session_data").join(filename), data)?;
        Ok(())
    }

    // Load the last used session filename.
    pub fn load_last_session_filename(&self) -> Option<String> {
        self.ensure_session_data_directory_exists();
        if let Ok(filename) = fs::read_to_string(self.base_dir.join("session_data/last_session.txt")) {
            return Some(filename);
        }
        None
    }

    // Save the last used session filename.
    pub fn save_last_session_filename(&self, filename: &str) -> Result<(), std::io::Error> {
        self.ensure_session_data_directory_exists();
        fs::write(self.base_dir.join("session_data/last_session.txt"), filename)?;
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

    pub fn save_ingested_data_log(&self, filename: &str, data: &str, chunk_num: usize, token_count: usize) -> Result<(), std::io::Error> {
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
    pub fn copy_ingested_file(&self, src_path: &PathBuf, filename: &str) -> Result<(), std::io::Error> {
        let dest_dir = self.base_dir.join(format!("session_data/ingested/{}_files", filename));
        if !dest_dir.exists() {
            fs::create_dir_all(&dest_dir)?;
        }
        
        let dest_path = dest_dir.join(src_path.file_name().unwrap());
        fs::copy(src_path, dest_path)?;
        Ok(())
    }
    
    pub fn handle_ingest(&self, path: &PathBuf) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let gpt = GPTConnector::new();
        let paths = if path.is_dir() {
            fs::read_dir(path)?
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![path.clone()]
        };
    
        let mut chunks: Vec<String> = Vec::new();
    
        for path in paths {
            let mut index = 1;
            loop {
                let (chunk, _total_chunks) = file_chunker::chunk_file(&path, index);
                if chunk == "Index out of bounds." {
                    break;
                }
    
                chunks.push(chunk.clone());
    
                let user_message = ChatCompletionRequestMessage {
                    role: Role::User,
                    content: chunk,
                };
    
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?
                    .block_on(gpt.send_request(vec![user_message]))?;
    
                index += 1;
            }
        }
    
        Ok(chunks)
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;
    use crate::pdf_extractor::PdfText;

    #[test]
    fn test_save_ingested_data_log() {
        let dir = tempdir().unwrap();
        let manager = SessionManager::new(dir.path().to_path_buf());
        let filename = "test_session";
        manager.save_ingested_data_log(filename, "test_data", 1, 500).unwrap();

        // Verify the file exists and has the expected content
        let log_path = dir.path().join("session_data/ingested/test_session_ingest.json");
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
        File::create(&src_path).unwrap().write_all(b"Hello, World!").unwrap();

        manager.copy_ingested_file(&src_path, "test_session").unwrap();

        let dest_path = dir.path().join("session_data/ingested/test_session_files/source.txt");
        assert!(dest_path.exists());
        let content = fs::read_to_string(dest_path).unwrap();
        assert_eq!(content, "Hello, World!");
    }
    #[test]
    fn test_handle_ingest() {
        let manager = SessionManager::new(PathBuf::from("tests/data"));
        let test_files = vec![
            "testText1.txt",
            "NIST.SP.800-185.pdf",
        ];
    
        for file_name in test_files.iter() {
            let path = manager.base_dir.join(file_name);
            manager.handle_ingest(&path).unwrap();
    
            // Validate each of the first four chunks
            for i in 1..=4 {
                // Validate log entry
                let log_path = manager.base_dir.join("ingested").join(format!("{}_ingest.json", i));
                assert!(log_path.exists(), "Log file for chunk {} does not exist for {}", i, file_name);
    
                // Extract chunked content
                let log_content = fs::read_to_string(&log_path).unwrap();
                let log_data: serde_json::Value = serde_json::from_str(&log_content).expect("Invalid JSON in log file");
                let chunk_file_path = log_data["file_path"].as_str().expect("Invalid file_path in log");
                let chunk_content = fs::read_to_string(chunk_file_path).expect("Failed to read chunked content");
    
                // Validate chunk content based on the original file
                if path.extension().unwrap_or_default() == "txt" {
                    let original_content = fs::read_to_string(&path).unwrap();
                    let expected_lines: Vec<&str> = original_content.lines().collect();
                    let expected_content = expected_lines[i-1];  // zero-indexed
                    assert_eq!(chunk_content.trim(), expected_content, "Chunk content mismatch for chunk {} of {}", i, file_name);
                } else {
                    // For PDF, we expect the content of a single page
                    let pdf_text = PdfText::from_pdf(&path).unwrap();
                    let expected_content = pdf_text.get_page_text(i as u32).expect("Failed to get page text");
                    assert_eq!(chunk_content.trim(), expected_content.join("\n"), "Chunk content mismatch for chunk {} of {}", i, file_name);
                }
    
                println!("Validated: Chunk content for chunk {} of {}", i, file_name);
            }
        }
    }
    
    #[test]
    fn integration_test_ingestion() {
        let dir = tempdir().unwrap();
        let manager = SessionManager::new(dir.path().to_path_buf());

        let txt_path = dir.path().join("test.txt");
        File::create(&txt_path).unwrap().write_all(b"Chunk 1\nChunk 2\nChunk 3").unwrap();

        let chunks = manager.handle_ingest(&txt_path).unwrap();

        // Verify ingestion
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "Chunk 1");
        assert_eq!(chunks[1], "Chunk 2");
        assert_eq!(chunks[2], "Chunk 3");

        // Verify ingested data log
        let log_path = dir.path().join("session_data/ingested/test_session_ingest.json");
        assert!(log_path.exists());
        let content = fs::read_to_string(log_path).unwrap();
        assert!(content.contains("\"chunk_num\":3"));

        // Verify copied file
        let dest_path = dir.path().join("session_data/ingested/test_session_files/test.txt");
        assert!(dest_path.exists());
        let file_content = fs::read_to_string(dest_path).unwrap();
        assert_eq!(file_content, "Chunk 1\nChunk 2\nChunk 3");
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
