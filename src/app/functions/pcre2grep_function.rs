use crate::app::functions::function_call::ModelFunction;
use std::process::Command;
use crate::app::functions::errors::FunctionError;
use serde_json::Value;

// This structure represents the options we allow for pcre2grep.
// Easy to add or remove elements for future updates.
const PCRE2GREP_OPTIONS: &[(char, &str, &str)] = &[
    // (Character Option, Full Option, Description)
    ('i', "--ignore-case", "Ignore case distinctions in the pattern."),
    ('v', "--invert-match", "Select non-matching lines."),
    ('l', "--files-with-matches", "Print only file names with matches."),
    ('c', "--count", "Print only a count of matching lines per file."),
    ('n', "--line-number", "Print line number with output lines."),
    ('e', "--regexp", "Specify a pattern, may be used more than once."),
    ('r', "--recursive", "Recursively scan sub-directories."),
    ('H', "--with-filename", "Force the prefixing of the file name on output."),
    ('h', "--no-filename", "Suppress the prefixing of the file name on output."),
    ('o', "--only-matching", "Show only the part of the line that matched."),
];

pub struct Pcre2GrepFunction {
    options: String,
    pattern: String,
    paths: String,
}

impl Pcre2GrepFunction {
    pub fn new(options: String, pattern: String, paths: String) -> Self {
        Self {
            options,
            pattern,
            paths,
        }
    }

    fn execute_pcre2grep(&self) -> Result<String, FunctionError> {
        let options = self.options.split(',')
            .filter_map(|opt| {
                let opt_trimmed = opt.trim();
                if opt_trimmed.is_empty() { None } else { Some(opt_trimmed) }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let output = Command::new("pcre2grep")
            .args(&[&options, &self.pattern])
            .args(self.paths.split(' '))
            .output()
            .map_err(|e| FunctionError::CommandExecutionError(e.to_string()))?;

        if !output.status.success() {
            return Err(FunctionError::CommandError(output.status.code()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    
    fn validate_options(&self) -> Result<(), FunctionError> {
        let valid_options: Vec<char> = PCRE2GREP_OPTIONS.iter().map(|opt| opt.0).collect();
        for opt in self.options.split(',') {
            let opt_trim = opt.trim();
            if !valid_options.contains(&opt_trim.chars().next().unwrap()) {
                return Err(FunctionError::InvalidArgument(format!("Invalid option '-{}'. Available options are: {:?}", opt, valid_options)));
            }
        }
        Ok(())
    }

    fn validate_paths(&self) -> Result<(), FunctionError> {
        // Implement logic to validate paths. For instance:
        if !self.paths.starts_with("./") {
            return Err(FunctionError::InvalidArgument("Paths must be relative and start with './'".to_owned()));
        }
        Ok(())
    }
}

impl ModelFunction for Pcre2GrepFunction {
    fn call(&self) -> Result<Value, FunctionError> {
        self.validate_options()?;
        self.validate_paths()?;
        
        let result = self.execute_pcre2grep()?;
        // Parse the result into the desired output format (e.g., JSON)
        let json_result = serde_json::from_str(&result)
            .map_err(|e| FunctionError::ParseError(e.to_string()))?;
        Ok(json_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_options_with_valid_option() {
        let pcre2_function = Pcre2GrepFunction::new("-i".to_string(), "pattern".to_string(), "./path".to_string());
        assert!(pcre2_function.validate_options().is_ok());
    }

    #[test]
    fn test_validate_options_with_invalid_option() {
        let pcre2_function = Pcre2GrepFunction::new("-z".to_string(), "pattern".to_string(), "./path".to_string());
        assert!(pcre2_function.validate_options().is_err());
    }

    #[test]
    fn test_validate_paths_with_valid_path() {
        let pcre2_function = Pcre2GrepFunction::new("-i".to_string(), "pattern".to_string(), "./valid/path".to_string());
        assert!(pcre2_function.validate_paths().is_ok());
    }

    #[test]
    fn test_validate_paths_with_invalid_path() {
        let pcre2_function = Pcre2GrepFunction::new("-i".to_string(), "pattern".to_string(), "invalid/path".to_string());
        assert!(pcre2_function.validate_paths().is_err());
    }

    // Additional tests for execute_pcre2grep can be added here
}
