use color_eyre::eyre::Result;

use async_openai::{
  config::OpenAIConfig,
  types::{ChatChoice, ChatCompletionFunctions, ChatCompletionRequestMessage, Role},
  Client,
};

use crate::app::types::*;

use std::{
  collections::HashMap,
  io::{BufRead, Write},
  path::PathBuf,
};

#[derive(Clone)]
pub struct GPTConnector {
  pub settings: GPTSettings,
  pub include_functions: bool,
  pub client: Client<OpenAIConfig>,
  pub model: Model,
}

pub fn define_commands() -> Vec<Command> {
  let mut commands: Vec<Command> = Vec::new();
  let command = Command {
    name: "list_dir".to_string(),
    description: Some("List directory contents".to_string()),
    parameters: Some(CommandParameters {
      param_type: "object".to_string(),
      required: vec!["path".to_string()],
      properties: HashMap::from([(
        "path".to_string(),
        CommandProperty {
          property_type: "string".to_string(),
          description: Some("path to directory".to_string()),
          enum_values: None,
        },
      )]),
    }),
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

pub fn list_dir(path: &str) -> Result<Option<String>, std::io::Error> {
  let mut dir_contents = String::new();
  for entry in std::fs::read_dir(path)? {
    let entry = entry?;
    let path = entry.path();
    let path_str = path.to_str().unwrap();
    dir_contents.push_str(path_str);
    dir_contents.push('\n');
  }
  Ok(Some(dir_contents))
}
#[tracing::instrument]
pub fn read_file_lines(
  path: &str,
  start_line: Option<usize>,
  end_line: Option<usize>,
) -> Result<Option<String>, std::io::Error> {
  let start_line = match start_line {
    Some(start_line) => start_line,
    None => 0,
  };
  let end_line = match end_line {
    Some(end_line) => end_line,
    None => {
      let file = std::fs::File::open(PathBuf::from(path)).unwrap();
      let reader = std::io::BufReader::new(file);
      reader.lines().count()
    },
  };
  let mut file_contents = String::new();
  let file = std::fs::File::open(path)?;
  let reader = std::io::BufReader::new(file);
  for (index, line) in reader.lines().enumerate() {
    if index >= start_line && index <= end_line {
      file_contents.push_str(&line?);
      file_contents.push('\n');
    }
  }
  Ok(Some(file_contents))
}

pub fn replace_lines(
  path: &str,
  start_line: Option<usize>,
  end_line: Option<usize>,
  replace_text: &str,
) -> Result<Option<String>, std::io::Error> {
  let mut file_contents = String::new();
  let end_line = match end_line {
    Some(end_line) => end_line,
    None => {
      let file = std::fs::File::open(path)?;
      let reader = std::io::BufReader::new(file);
      reader.lines().count()
    },
  };
  let start_line = match start_line {
    Some(start_line) => start_line,
    None => 0,
  };
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

pub fn cargo_check() -> Result<Option<String>, std::io::Error> {
  let mut command = std::process::Command::new("cargo");
  command.arg("check");
  let output = command.output()?;
  println!("{}", String::from_utf8_lossy(&output.stdout));
  Ok(None)
}

pub fn create_chat_completion_function_args(commands: Vec<Command>) -> Vec<ChatCompletionFunctions> {
  let mut chat_completion_functions: Vec<ChatCompletionFunctions> = Vec::new();
  for command in commands {
    let chat_completion_function = ChatCompletionFunctions {
      name: command.name,
      description: command.description,
      parameters: Some(serde_json::to_value(command.parameters).unwrap()),
    };
    chat_completion_functions.push(chat_completion_function);
  }
  chat_completion_functions
}

pub fn handle_chat_response_function_call(
  response_choices: Vec<ChatChoice>,
) -> Option<Vec<ChatCompletionRequestMessage>> {
  let mut function_results: Vec<ChatCompletionRequestMessage> = Vec::new();
  // println!("response_choices: {:?}", response_choices);
  for choice in response_choices {
    if let Some(function_call) = choice.message.function_call {
      let function_name = function_call.name;
      let function_args: serde_json::Value = function_call.arguments.parse().unwrap();
      let function_call_result: Result<Option<String>, std::io::Error> = match function_name.as_str() {
        "list_dir" => list_dir(function_args["path"].as_str().unwrap()),
        "read_lines" => read_file_lines(
          function_args["path"].as_str().unwrap(),
          Some(function_args["start_line"].as_u64().unwrap_or_default() as usize),
          Some(function_args["end_line"].as_u64().unwrap_or_default() as usize),
        ),
        "replace_lines" => replace_lines(
          function_args["path"].as_str().unwrap(),
          Some(function_args["start_line"].as_u64().unwrap() as usize),
          Some(function_args["end_line"].as_u64().unwrap() as usize),
          function_args["replace_text"].as_str().unwrap(),
        ),
        "cargo_check" => cargo_check(),
        _ => Ok(None),
      };
      match function_call_result {
        Ok(Some(output)) => {
          function_results.push(ChatCompletionRequestMessage {
            name: Some("Sazid".to_string()),
            role: Role::Function,
            content: Some(output),
            ..Default::default()
          });
        },
        Ok(None) => {},
        Err(e) => {
          function_results.push(ChatCompletionRequestMessage {
            name: Some("Sazid".to_string()),
            role: Role::Function,
            content: Some(format!("Error: {:?}", e)),
            ..Default::default()
          });
        },
      }
    }
  }
  if function_results.len() > 0 {
    Some(function_results)
  } else {
    None
  }
}
#[cfg(test)]
mod test {

  #[test]
  fn test_list_dir() {
    let dir_contents = super::list_dir("./src");
    assert!(dir_contents.is_ok());
  }

  #[test]
  fn test_read_file_lines() {
    let file_contents = super::read_file_lines("./src/gpt_commands.rs", Some(0), Some(10));
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

