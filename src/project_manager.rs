use std::fs;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use std::io;

#[derive(Serialize, Deserialize, Debug)]
pub struct Project {
    name: String,
    project_type: String,
    goals: String,
    path: PathBuf,
}

impl Project {
    pub fn new(name: &str, project_type: &str, goals: &str, path: PathBuf) -> Self {
        Project {
            name: name.to_string(),
            project_type: project_type.to_string(),
            goals: goals.to_string(),
            path,
        }
    }

    pub fn get_path(&self) -> &PathBuf {
        &self.path
    }

    pub fn ingest_file(&self, file_path: &PathBuf) -> io::Result<()> {
        let dest_path = self.path.join("workspace_data").join(file_path.file_name().unwrap());
        fs::copy(file_path, dest_path)?;
        Ok(())
    }

    pub fn create_project(name: &str, project_type: &str, goals: &str, path: PathBuf) -> Self {
        let project = Project::new(name, project_type, goals, path);
        fs::create_dir_all(project.path.join("workspace_data")).unwrap();
        fs::create_dir_all(project.path.join("logs")).unwrap();
        project
    }

    pub fn load_project(path: PathBuf) -> Option<Self> {
        if path.exists() {
            let project_data = fs::read_to_string(path.join("config.json")).ok()?;
            serde_json::from_str(&project_data).ok()
        } else {
            None
        }
    }

    pub fn delete_project(&self) -> io::Result<()> {
        fs::remove_dir_all(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::path::Path;
    
    #[test]
    fn test_project_creation() {
        let project = Project::create_project("TestProject", "Type", "Goals", PathBuf::from("TestProject"));
        assert!(Path::new("TestProject").exists());
        assert!(Path::new("TestProject/workspace_data").exists());
        assert!(Path::new("TestProject/logs").exists());
        project.delete_project().unwrap();
    }

    #[test]
    fn test_project_loading() {
        let project = Project::create_project("TestProject", "Type", "Goals", PathBuf::from("TestProject"));
        let loaded_project = Project::load_project(PathBuf::from("TestProject"));
        assert!(loaded_project.is_some());
        assert_eq!(loaded_project.unwrap().name, "TestProject");
        project.delete_project().unwrap();
    }

    #[test]
    fn test_file_ingestion() {
        let project = Project::create_project("TestProject", "Type", "Goals", PathBuf::from("TestProject"));
        File::create("TestProject/workspace_data/test.txt").unwrap();
        project.ingest_file(&PathBuf::from("test.txt")).unwrap();
        assert!(!Path::new("TestProject/workspace_data/test.txt").exists());
        assert!(Path::new("TestProject/workspace_data/test.txt").exists());
        project.delete_project().unwrap();
    }

    #[test]
    fn test_project_deletion() {
        let project = Project::create_project("TestProject", "Type", "Goals", PathBuf::from("TestProject"));
        project.delete_project().unwrap();
        assert!(!Path::new("TestProject").exists());
    }
}
