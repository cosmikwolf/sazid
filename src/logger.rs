use std::fs::OpenOptions;
use std::io::prelude::*;
use std::path::PathBuf;
use chrono::prelude::*;

pub struct Logger {
    log_dir: PathBuf,
}

impl Logger {
    pub fn new() -> Logger {
        let log_dir = PathBuf::from("logs");
        if !log_dir.exists() {
            std::fs::create_dir_all(&log_dir).unwrap();
        }
        Logger { log_dir }
    }

    pub fn log_interaction(&self, request: &str, response: &str) {
        let datetime = Utc::now();
        let log_file_name = datetime.format("%Y-%m-%d_%H-%M-%S.log").to_string();
        let log_file_path = self.log_dir.join(log_file_name);

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(log_file_path)
            .unwrap();

        let log_content = format!("Request: {}\nResponse: {}", request, response);
        file.write_all(log_content.as_bytes()).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_log_interaction() {
        let logger = Logger::new();
        logger.log_interaction("Hello, GPT!", "Hi, User!");
        assert!(fs::read_dir("logs").unwrap().count() > 0); // Ensure there's at least one log

        // Clean up the logs for next tests
        for entry in fs::read_dir("logs").unwrap() {
            let dir = entry.unwrap();
            fs::remove_file(dir.path()).unwrap();
        }
    }
}
