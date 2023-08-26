use std::fs::{OpenOptions, File};
use std::io::prelude::*;
use std::path::PathBuf;
use chrono::prelude::*;

pub struct Logger {
    project_path: PathBuf,
}

impl Logger {
    pub fn new(project_path: PathBuf) -> Logger {
        Logger {
            project_path,
        }
    }

    pub fn log(&self, message: &str, log_type: &str) {
        let log_dir = self.project_path.join("logs");
        if !log_dir.exists() {
            std::fs::create_dir_all(&log_dir).unwrap();
        }

        let log_file = log_dir.join(format!("{}.log", log_type));
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(log_file)
            .unwrap();

        let datetime = Utc::now();
        let log_message = format!("[{}]: {}\n", datetime, message);
        file.write_all(log_message.as_bytes()).unwrap();
    }

    pub fn log_token_usage(&self, tokens_used: usize) {
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let log_message = format!("{}: {} tokens\n", date, tokens_used);
        self.log(&log_message, "usage");
    }

    pub fn clear_logs(&self, log_type: &str) {
        let log_file = self.project_path.join("logs").join(format!("{}.log", log_type));
        File::create(log_file).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_log() {
        let project_path = PathBuf::from("test_project");
        let logger = Logger::new(project_path.clone());
        logger.log("Test message", "chat");
        let log_file = project_path.join("logs").join("chat.log");
        let contents = fs::read_to_string(&log_file).unwrap();
        assert!(contents.contains("Test message"));
        fs::remove_file(log_file).unwrap();
        fs::remove_dir_all(project_path.join("logs")).unwrap();
        fs::remove_dir(project_path).unwrap();
    }

    #[test]
    fn test_log_token_usage() {
        let project_path = PathBuf::from("test_project");
        let logger = Logger::new(project_path.clone());
        logger.log_token_usage(500);
        let log_file = project_path.join("logs").join("usage.log");
        let contents = fs::read_to_string(&log_file).unwrap();
        assert!(contents.contains("500 tokens"));
        fs::remove_file(log_file).unwrap();
        fs::remove_dir_all(project_path.join("logs")).unwrap();
        fs::remove_dir(project_path).unwrap();
    }

    #[test]
    fn test_clear_logs() {
        let project_path = PathBuf::from("test_project");
        let logger = Logger::new(project_path.clone());
        logger.log("Test message", "chat");
        logger.clear_logs("chat");
        let log_file = project_path.join("logs").join("chat.log");
        let contents = fs::read_to_string(&log_file).unwrap();
        assert_eq!(contents, "");
        fs::remove_file(log_file).unwrap();
        fs::remove_dir_all(project_path.join("logs")).unwrap();
        fs::remove_dir(project_path).unwrap();
    }
}
