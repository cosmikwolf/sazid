use std::fs;
use std::path::Path;
use std::io::{self, Write};

pub struct Project {
    name: String,
    description: String,
    goals: String,
    role: String,
    path: String,
}

impl Project {
    pub fn new(name: &str, description: &str, goals: &str, role: &str) -> Self {
        let path = format!("./{}", name);
        Self {
            name: name.to_string(),
            description: description.to_string(),
            goals: goals.to_string(),
            role: role.to_string(),
            path,
        }
    }

    pub fn create(&self) -> io::Result<()> {
        fs::create_dir_all(&self.path)?;
        fs::create_dir_all(format!("{}/ingest", &self.path))?;
        fs::create_dir_all(format!("{}/workspace_data", &self.path))?;
        fs::create_dir_all(format!("{}/logs", &self.path))?;

        let mut config = fs::File::create(format!("{}/config.toml", &self.path))?;
        write!(config, "description = \"{}\"\ngoals = \"{}\"\nrole = \"{}\"", self.description, self.goals, self.role)?;

        Ok(())
    }


    pub fn list_projects() -> io::Result<Vec<String>> {
        let entries = fs::read_dir(".")?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                if entry.path().is_dir() {
                    Some(entry.file_name().into_string().ok()?)
                } else {
                    None
                }
            })
            .collect();
        Ok(entries)
    }

    pub fn delete(name: &str) -> io::Result<()> {
        if Path::new(name).exists() {
            fs::remove_dir_all(name)?;
        }
        Ok(())
    }

    pub fn load_project(name: &str) -> io::Result<Self> {
        let path = format!("./{}", name);
        if !Path::new(&path).exists() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Project not found"));
        }
        let config_content = fs::read_to_string(format!("{}/config.toml", path))?;
        // For simplicity, we'll just split the content. In a real-world scenario, you'd use a TOML parser.
        let lines: Vec<&str> = config_content.lines().collect();
        let description = lines[0].split('=').last().unwrap().trim_matches('"').to_string();
        let goals = lines[1].split('=').last().unwrap().trim_matches('"').to_string();
        let role = lines[2].split('=').last().unwrap().trim_matches('"').to_string();

        Ok(Self {
            name: name.to_string(),
            description,
            goals,
            role,
            path,
        })
    }

    pub fn save_chat_log(session_log: Vec<String>) {
        // Determine the project's log folder
        let log_path = "path_to_project_folder/logs"; // Update this with the actual path
    
        // Generate a unique filename for the chat session
        let filename = format!("{}/chat_{}.log", log_path, chrono::Utc::now().format("%Y%m%d%H%M%S"));
    
        // Save the chat log
        std::fs::write(&filename, session_log.join("\n")).expect("Unable to write chat log");
    }
    
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_creation() {
        let project = Project::new("TestProject", "Test Description", "Test Goals", "Test Role");
        project.create().unwrap();
        assert!(Path::new("./TestProject").exists());
        assert!(Path::new("./TestProject/ingest").exists());
        assert!(Path::new("./TestProject/workspace_data").exists());
        assert!(Path::new("./TestProject/logs").exists());
        assert!(Path::new("./TestProject/config.toml").exists());
        // Cleanup after test
        fs::remove_dir_all("./TestProject").unwrap();
    }

    #[test]
    fn test_project_list() {
        let project = Project::new("TestProject", "Test Description", "Test Goals", "Test Role");
        project.create().unwrap();
        let projects = Project::list_projects().unwrap();
        assert!(projects.contains(&"TestProject".to_string()));
        // Cleanup after test
        fs::remove_dir_all("./TestProject").unwrap();
    }

    #[test]
    fn test_project_delete() {
        let project = Project::new("TestProject", "Test Description", "Test Goals", "Test Role");
        project.create().unwrap();
        Project::delete("TestProject").unwrap();
        assert!(!Path::new("./TestProject").exists());
    }

    #[test]
    fn test_project_load() {
        let project = Project::new("TestProject", "Test Description", "Test Goals", "Test Role");
        project.create().unwrap();
        let loaded_project = Project::load_project("TestProject").unwrap();
        assert_eq!(loaded_project.name, "TestProject");
        assert_eq!(loaded_project.description, "Test Description");
        assert_eq!(loaded_project.goals, "Test Goals");
        assert_eq!(loaded_project.role, "Test Role");
        // Cleanup after test
        fs::remove_dir_all("./TestProject").unwrap();
    }
    // Additional tests for other project management functions...
}
