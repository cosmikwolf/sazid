use crate::types::*;
use crate::utils;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const SESSIONS_DIR: &str = "data/sessions";
pub const INGESTED_DIR: &str = "data/ingested";


impl SessionManager {
    pub fn new(
        settings: GPTSettings,
        include_functions: bool,
        session_data: Option<Session>,
    ) -> SessionManager {
        let mut session_data = match session_data {
            Some(session_data) => session_data,
            None => Session::new(utils::generate_session_id() , settings, include_functions),
        };
        Self {
            include_functions,
            cached_request: None,
            session_data,
        }
    }

    pub fn save_session(&self) -> io::Result<()> {
        utils::ensure_directory_exists(SESSIONS_DIR).unwrap();
        let session_file_path = self.get_session_filepath();
        let data = serde_json::to_string(&self.session_data)?;
        fs::write(session_file_path, data)?;
        self.save_last_session_file_path();
        Ok(())
    }

    pub fn get_last_session_file_path() -> Option<PathBuf> {
        utils::ensure_directory_exists(SESSIONS_DIR).unwrap();
        let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
        if last_session_path.exists() {
            Some(fs::read_to_string(last_session_path).unwrap().into())
        } else {
            None
        }
    }

    // list all sessions in the sessions directory
    pub fn list_sessions() -> io::Result<Vec<PathBuf>> {
        utils::ensure_directory_exists(SESSIONS_DIR)?;
        let mut sessions: Vec<PathBuf> = Vec::new();
        for entry in fs::read_dir(SESSIONS_DIR)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                sessions.push(path);
            }
        }
        Ok(sessions)
    }

    pub fn save_last_session_file_path(&self) {
        utils::ensure_directory_exists(SESSIONS_DIR).unwrap();
        let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
        fs::write(
            last_session_path,
            self.get_session_filepath().display().to_string(),
        )
        .unwrap();
    }

    pub fn get_session_filepath(&self) -> PathBuf {
        Path::new(SESSIONS_DIR).join(self.get_session_filename())
    }

    pub fn get_session_filename(&self) -> String {
        format!("{}.json", self.session_data.session_id)
    }

    pub fn get_ingested_filepath(&self) -> PathBuf {
        Path::new(INGESTED_DIR).join(format!("{}.json", self.session_data.session_id))
    }

}

// Tests
#[cfg(test)]
mod tests {}
