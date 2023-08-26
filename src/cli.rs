use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub enum CliArgs {
    /// Create a new project
    NewProject {
        name: String,
    },
    /// Ingest a file into the current project
    IngestFile {
        file_path: String,
    },
    /// Start a chat session with GPT
    Start,
}

impl CliArgs {
    pub fn parse() -> Self {
        CliArgs::from_args()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let args = vec!["voracious", "NewProject", "TestProject"];
        let parsed_args = CliArgs::from_iter_safe(args).unwrap();
        match parsed_args {
            CliArgs::NewProject { name } => assert_eq!(name, "TestProject"),
            _ => panic!("Unexpected command parsed"),
        }
    }
}
