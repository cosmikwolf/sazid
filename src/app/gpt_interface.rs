use async_openai::{config::OpenAIConfig, types::ChatCompletionFunctions, Client};
use tiktoken_rs::cl100k_base;

use crate::{app::types::*, trace_dbg};
use walkdir::WalkDir;

use std::{
  collections::HashMap,
  fs::{File, OpenOptions},
  io::{BufRead, BufReader, Write},
  path::{Path, PathBuf},
};

use super::errors::FunctionCallError;
use rust_fuzzy_search;

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

fn count_lines_and_format_search_results(
  path: &str,
  column_width: usize,
  result_score: Option<&f32>,
) -> Option<String> {
  if !Path::new(path).is_file() {
    return None;
  }
  match File::open(path) {
    Ok(file) => {
      trace_dbg!(path);
      let reader = BufReader::new(file);
      let linecount = reader.lines().count();
      trace_dbg!("debug 3");
      // format line that is below, but truncates s.1 to 2 decimal places
      match result_score {
        Some(score) => Some(format!("{:column_width$}\t{:<15.2}\t{} lines", path, score, linecount)),
        None => Some(format!("{:column_width$}\t{} lines", path, linecount)),
      }
    },
    Err(e) => Some(format!("error opening file path {} error: {}", path, e)),
  }
}

fn get_column_width(strings: Vec<&str>) -> usize {
  strings.iter().map(|s| s.len()).max().unwrap_or(0) + 2
}

pub fn file_search(
  reply_max_tokens: usize,
  list_file_paths: Vec<PathBuf>,
  search_term: Option<&str>,
) -> Result<Option<String>, FunctionCallError> {
  let paths = get_accessible_file_paths(list_file_paths);
  let accessible_paths = paths.keys().map(|path| path.as_str()).collect::<Vec<&str>>();
  // find the length of the longest string in accessible_paths
  let search_results = if let Some(search) = search_term {
    trace_dbg!("searching with term: {}", search);
    let fuzzy_search_result = rust_fuzzy_search::fuzzy_search_threshold(search, &accessible_paths, 0.1);
    let column_width = get_column_width(fuzzy_search_result.iter().map(|(s, _)| *s).collect());
    let fuzzy_search_result = fuzzy_search_result
      .iter()
      .filter(|(_, result_score)| result_score > &0.1)
      .filter_map(|(path, result_score)| count_lines_and_format_search_results(path, column_width, Some(result_score)))
      .collect::<Vec<String>>();
    if fuzzy_search_result.is_empty() {
      return Ok(Some("no files matching search term found".to_string()));
    } else {
      fuzzy_search_result.join("\n")
    }
  } else {
    trace_dbg!("searching without a search term");
    if accessible_paths.is_empty() {
      return Ok(Some("no files are accessible. User must add files to the search path configuration".to_string()));
    } else {
      let column_width = get_column_width(accessible_paths.clone());
      accessible_paths
        .iter()
        .filter_map(|s| count_lines_and_format_search_results(s, column_width, None))
        .collect::<Vec<String>>()
        .join("\n")
    }
  };
  let token_count = count_tokens(&search_results);
  if token_count > reply_max_tokens {
    return Ok(Some(format!("Function Token limit exceeded: {} tokens.", token_count)));
  }
  Ok(Some(search_results))
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
        return Err(FunctionCallError::new(
          format!("Error reading file: {}\nare you sure a file exists at the path you are accessing?", error).as_str(),
        ));
      },
    };

    // individually validate start_line and end_line and make sure that if they are Some(value) that they are within the respective bounds of the file

    if let Some(start_line) = start_line {
      if start_line > file_contents.len() {
        return Err(FunctionCallError::new("Invalid start line number."));
      }
    }

    if let Some(end_line) = end_line {
      if end_line > file_contents.len() {
        return Err(FunctionCallError::new("Invalid end line number."));
      }
    }
    let selected_lines: Vec<String> =
      file_contents[start_line.unwrap_or(0)..end_line.unwrap_or(file_contents.len())].to_vec();
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
    Err(FunctionCallError::new(
      "File not found or not accessible.\nare you sure a file exists at the path you are accessing?",
    ))
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
    name: "create_file".to_string(),
    description: Some("create a text file".to_string()),
    parameters: Some(CommandParameters {
      param_type: "object".to_string(),
      required: vec![],
      properties: HashMap::from([
        (
          "path".to_string(),
          CommandProperty {
            property_type: "string".to_string(),
            description: Some("path to create file. all file paths must start with ./".to_string()),
            enum_values: None,
          },
        ),
        (
          "text".to_string(),
          CommandProperty {
            property_type: "string".to_string(),
            description: Some("text to enter into file".to_string()),
            enum_values: None,
          },
        ),
      ]),
    }),
  };
  commands.push(command);
  let command = Command {
    name: "file_search".to_string(),
    description: Some(
      "search accessible file paths. file_search without arguments returns all accessible file paths. results include file line count".to_string(),
    ),
    parameters: Some(CommandParameters {
      param_type: "object".to_string(),
      required: vec![],
      properties: HashMap::from([(
        "search_term".to_string(),
        CommandProperty {
          property_type: "string".to_string(),
          description: Some(
            "fuzzy search for files by name or path. search results contain a match score and line count.".to_string(),
          ),
          enum_values: None,
        },
      )]),
    }),
  };
  commands.push(command);
  let command = Command {
    name: "read_lines".to_string(),
    description: Some("read lines from an accessible file path".to_string()),
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
            description: Some("line to start read - omit to read file from beginning".to_string()),
            enum_values: None,
          },
        ),
        (
          "end_line".to_string(),
          CommandProperty {
            property_type: "number".to_string(),
            description: Some("line to end read - omit to read file to EOF".to_string()),
            enum_values: None,
          },
        ),
      ]),
    }),
  };
  commands.push(command);
  let command = Command {
    name: "modify_file".to_string(),
    description: Some("modify a file by adding, removing, or replacing lines of text".to_string()),
    parameters: Some(CommandParameters {
      param_type: "object".to_string(),
      required: vec!["path".to_string(), "start_line".to_string()],
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
            description: Some("line number to begin add and remove".to_string()),
            enum_values: None,
          },
        ),
        (
          "end_line".to_string(),
          CommandProperty {
            property_type: "number".to_string(),
            description: Some("last line to remove, starting at start_line. Omit end_line to insert text at starting line without removal".to_string()),
            enum_values: None,
          },
        ),
        (
          "insert_text".to_string(),
          CommandProperty {
            property_type: "string".to_string(),
            description: Some("text to insert at start_line, after optional removal".to_string()),
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
pub fn create_file(path: &str, text: &str) -> Result<Option<String>, FunctionCallError> {
  match File::create(path) {
    Ok(mut file) => match file.write_all(text.as_bytes()) {
      Ok(_) => Ok(Some("file created".to_string())),
      Err(e) => Ok(Some(format!("error writing file: {}", e))),
    },
    Err(e) => Ok(Some(format!("error creating file at {}, error: {}", path, e))),
  }
}

pub fn modify_file(
  path: &str,
  start_line: usize,
  end_line: Option<usize>,
  insert_text: Option<&str>,
) -> Result<Option<String>, FunctionCallError> {
  match File::open(path) {
    Ok(file) => {
      let reader = std::io::BufReader::new(&file);
      let mut new_lines: Vec<String> = reader.lines().map(|line| line.unwrap_or_default().to_string()).collect();
      let file_line_count = new_lines.len();
      if let Some(end_line) = end_line {
        if end_line > file_line_count {
          return Ok(Some("start_line + remove_line_count exceeds file length".to_string()));
        } else {
          // remove lines
          new_lines = new_lines
            .iter()
            .enumerate()
            .filter(|&(i, _)| i + 1 < start_line || i + 1 > end_line)
            .map(|(_, line)| line.to_string())
            .collect::<Vec<String>>();
        }
      }
      if let Some(insert_text) = insert_text {
        new_lines.insert(start_line, insert_text.to_string());
      }
      match OpenOptions::new().write(true).truncate(true).open(path) {
        Ok(mut file) => match file.write_all(new_lines.join("\n").as_bytes()) {
          Ok(_) => {
            let removed_string = if let Some(end_line) = end_line {
              format!("{} lines removed ", end_line - start_line)
            } else {
              "".to_string()
            };
            let added_string = if let Some(insert_text) = insert_text {
              format!("{} lines added ", insert_text.lines().count())
            } else {
              "".to_string()
            };
            Ok(Some(format!("success! {}{}at {}", removed_string, added_string, path)))
          },
          Err(e) => {
            let message = format!("error writing file modifications: {}", e);
            trace_dbg!("{}", message);
            Ok(Some(message))
          },
        },
        Err(e) => Ok(Some(format!("error opening file at {}, error: {}", path, e))),
      }
    },
    Err(e) => Ok(Some(format!("error opening file at {}, error: {}", path, e))),
  }
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
  let empty_parameters = "{\"type\": \"object\", \"properties\": {}}";
  for command in commands {
    let chat_completion_function = ChatCompletionFunctions {
      name: command.name,
      description: command.description,
      parameters: match command.parameters {
        Some(parameters) => Some(serde_json::to_value(parameters).unwrap()),
        None => Some(serde_json::from_str(empty_parameters).unwrap()),
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
    let dir_contents = super::file_search(1024, vec![PathBuf::from("src".to_string())], None);
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
