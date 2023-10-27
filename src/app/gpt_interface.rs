use color_eyre::eyre::Result;

use async_openai::{config::OpenAIConfig, types::ChatCompletionFunctions, Client};
use tiktoken_rs::cl100k_base;

use crate::{app::types::*, trace_dbg};
use walkdir::WalkDir;

use std::{
  collections::HashMap,
  fs::File,
  io::{BufRead, BufReader, Write},
  path::{Path, PathBuf},
};

use super::errors::FunctionCallError;

#[derive(Clone)]
pub struct GPTConnector {
  pub settings: GPTSettings,
  pub include_functions: bool,
  pub client: Client<OpenAIConfig>,
  pub model: Model,
}

pub fn get_accessible_file_paths(list_file_paths: Vec<PathBuf>) -> HashMap<String, PathBuf> {
  // Define the base directory you want to start the search from.
  let base_dir = PathBuf::from("./");

  // Create an empty HashMap to store the relative paths.
  let mut file_paths = HashMap::new();
  for mut path in list_file_paths {
    // Iterate through the files using WalkDir.
    path = base_dir.join(path);
    if path.exists() {
      WalkDir::new(path).into_iter().flatten().for_each(|entry| {
        let path = entry.path();
        file_paths.insert(path.to_string_lossy().to_string(), path.to_path_buf());
      });
    }
  }

  trace_dbg!("file_paths: {:?}", file_paths);
  file_paths
}

pub fn list_files(reply_max_tokens: usize, list_file_paths: Vec<PathBuf>) -> Result<Option<String>, FunctionCallError> {
  let paths = get_accessible_file_paths(list_file_paths);
  trace_dbg!("path count: {}", paths.len());
  let concatenated_paths: String = paths.keys().map(|path| path.to_string()).collect::<Vec<String>>().join("\n");
  let token_count = count_tokens(&concatenated_paths);
  if token_count > reply_max_tokens {
    return Ok(Some(format!("Function Token limit exceeded: {} tokens.", token_count)));
  }
  Ok(Some(concatenated_paths))
}

pub fn read_file_lines(
  file: &str,
  start_line: Option<usize>,
  end_line: Option<usize>,
  reply_max_tokens: usize,
  list_file_paths: Vec<PathBuf>,
) -> Result<Option<String>, FunctionCallError> {
  trace_dbg!("list_file_paths: {:?}", list_file_paths);
  trace_dbg!("file: {:?} {:#?}", get_accessible_file_paths(list_file_paths.clone()).get(file), file);
  if let Some(file_path) = get_accessible_file_paths(list_file_paths).get(file) {
    let file_contents = match read_lines(file_path) {
      Ok(contents) => contents,
      Err(error) => {
        return Err(FunctionCallError::new(format!("Error reading file: {}", error).as_str()));
      },
    };

    if let (Some(start), Some(end)) = (start_line, end_line) {
      // Ensure start and end lines are within bounds.
      if start > file_contents.len() || end > file_contents.len() || start > end {
        return Err(FunctionCallError::new("Invalid start or end line numbers."));
      }

      let selected_lines: Vec<String> = file_contents[start - 1..end].to_vec();
      let output = selected_lines.join("\n");

      let token_count = count_tokens(&output);
      if token_count > reply_max_tokens {
        return Ok(Some(format!("Function Token limit exceeded: {} tokens.", token_count)));
      }

      Ok(Some(format!(
        "----------\nFile: {}\nSize: {} lines\n{}\n-----------\n{}",
        file,
        file_contents.len(),
        output,
        token_count
      )))
    } else {
      Ok(Some(format!("File: {}\nSize: {} lines\n{}", file, file_contents.len(), file_contents.join("\n"))))
    }
  } else {
    Err(FunctionCallError::new("File not found or not accessible."))
  }
}

fn read_lines(file_path: &Path) -> Result<Vec<String>, std::io::Error> {
  let file = File::open(file_path)?;
  let reader = BufReader::new(file);
  reader.lines().collect()
}

fn count_tokens(text: &str) -> usize {
  let bpe = cl100k_base().unwrap();
  bpe.encode_with_special_tokens(text).len()
}

pub fn define_commands() -> Vec<Command> {
  let mut commands: Vec<Command> = Vec::new();
  // let command = Command {
  //   name: "list_dir".to_string(),
  //   description: Some("List directory contents".to_string()),
  //   parameters: Some(CommandParameters {
  //     param_type: "object".to_string(),
  //     required: vec!["path".to_string()],
  //     properties: HashMap::from([(
  //       "path".to_string(),
  //       CommandProperty {
  //         property_type: "string".to_string(),
  //         description: Some("path to directory".to_string()),
  //         enum_values: None,
  //       },
  //     )]),
  //   }),
  // };
  let command = Command {
    name: "list_files".to_string(),
    description: Some("List files that are accessible to this session".to_string()),
    parameters: None,
  };
  commands.push(command);
  let command = Command {
    name: "read_lines".to_string(),
    description: Some("read lines from a file, with optional start and end lines".to_string()),
    parameters: Some(CommandParameters {
      param_type: "object".to_string(),
      required: vec!["path".to_string()],
      properties: HashMap::from([
        (
          "path".to_string(),
          CommandProperty {
            property_type: "string".to_string(),
            description: Some("path to file".to_string()),
            enum_values: None,
          },
        ),
        (
          "start_line".to_string(),
          CommandProperty {
            property_type: "number".to_string(),
            description: Some("line to start read".to_string()),
            enum_values: None,
          },
        ),
        (
          "end_line".to_string(),
          CommandProperty {
            property_type: "number".to_string(),
            description: Some("line to end read".to_string()),
            enum_values: None,
          },
        ),
      ]),
    }),
  };
  commands.push(command);
  let command = Command {
    name: "replace_lines".to_string(),
    description: Some("replace lines in a text file".to_string()),
    parameters: Some(CommandParameters {
      param_type: "object".to_string(),
      required: vec!["path".to_string(), "start_line".to_string(), "end_line".to_string(), "replace_text".to_string()],
      properties: HashMap::from([
        (
          "path".to_string(),
          CommandProperty {
            property_type: "string".to_string(),
            description: Some("path to file".to_string()),
            enum_values: None,
          },
        ),
        (
          "start_line".to_string(),
          CommandProperty {
            property_type: "number".to_string(),
            description: Some("line to start replace".to_string()),
            enum_values: None,
          },
        ),
        (
          "end_line".to_string(),
          CommandProperty {
            property_type: "number".to_string(),
            description: Some("line to end replace".to_string()),
            enum_values: None,
          },
        ),
        (
          "replace_text".to_string(),
          CommandProperty {
            property_type: "string".to_string(),
            description: Some("text to replace removed lines".to_string()),
            enum_values: None,
          },
        ),
      ]),
    }),
  };
  commands.push(command);
  // let command = Command {
  //     name: "cargo check".to_string(),
  //     description: Some("run cargo check to discover any compilation errors".to_string()),
  //     parameters: None,
  // };
  // commands.push(command);
  commands
}
pub fn replace_lines(
  path: &str,
  start_line: Option<usize>,
  end_line: Option<usize>,
  replace_text: &str,
) -> Result<Option<String>, FunctionCallError> {
  let mut file_contents = String::new();
  let end_line = match end_line {
    Some(end_line) => end_line,
    None => {
      let file = std::fs::File::open(path)?;
      let reader = std::io::BufReader::new(file);
      reader.lines().count()
    },
  };
  let start_line = start_line.unwrap_or(0);
  let file = std::fs::File::open(path)?;
  let reader = std::io::BufReader::new(file);
  for (index, line) in reader.lines().enumerate() {
    if index >= start_line && index <= end_line {
      file_contents.push_str(&line?);
      file_contents.push('\n');
    }
  }
  let mut file = std::fs::File::create(path)?;
  file.write_all(replace_text.as_bytes())?;
  Ok(None)
}

pub fn cargo_check() -> Result<Option<String>, FunctionCallError> {
  let mut command = std::process::Command::new("cargo");
  command.arg("check");
  let output = command.output()?;
  println!("{}", String::from_utf8_lossy(&output.stdout));
  Ok(None)
}

pub fn create_chat_completion_function_args(commands: Vec<Command>) -> Vec<ChatCompletionFunctions> {
  let mut chat_completion_functions: Vec<ChatCompletionFunctions> = Vec::new();
  let string = "{\"type\": \"object\", \"properties\": {}}";
  for command in commands {
    let chat_completion_function = ChatCompletionFunctions {
      name: command.name,
      description: command.description,
      parameters: match command.parameters {
        Some(parameters) => Some(serde_json::to_value(parameters).unwrap()),
        None => Some(serde_json::from_str(string).unwrap()),
      },
    };
    chat_completion_functions.push(chat_completion_function);
  }
  chat_completion_functions
}

#[cfg(test)]
mod test {
  use std::path::PathBuf;

  #[test]
  fn test_list_dir() {
    let dir_contents = super::list_files(1024, vec![PathBuf::from("src".to_string())]);
    assert!(dir_contents.is_ok());
  }

  #[test]
  fn test_read_file_lines() {
    let file_contents =
      super::read_file_lines("./src/gpt_commands.rs", Some(0), Some(10), 1024, vec![PathBuf::from("src".to_string())]);
    assert!(file_contents.is_ok());
  }

  #[test]
  fn test_replace_lines() {
    // let replace_result = super::replace_lines("./src/gpt_commands.rs", 0, 10, "test");
    // assert!(replace_result.is_ok());
  }

  #[test]
  fn test_cargo_check() {
    let cargo_check_result = super::cargo_check();
    assert!(cargo_check_result.is_ok());
  }
}
