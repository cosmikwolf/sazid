use crate::{action::Action, trace_dbg};
use grep;
use std::{collections::HashMap, error::Error, fmt, io, path::PathBuf};
use tokio::sync::mpsc::UnboundedSender;
use walkdir::WalkDir;

use self::{
  cargo_check_function::CargoCheckFunction, create_file_function::CreateFileFunction,
  file_search_function::FileSearchFunction, grep_function::GrepFunction, modify_file_function::ModifyFileFunction,
  read_file_lines_function::ReadFileLinesFunction, types::Command,
};

use super::{
  session_config::SessionConfig,
  types::{ChatMessage, FunctionResult},
};

pub mod cargo_check_function;
pub mod create_file_function;
pub mod file_search_function;
pub mod grep_function;
pub mod modify_file_function;
pub mod patch_files_function;
pub mod read_file_lines_function;
pub mod types;

#[derive(Debug)]
pub struct FunctionCallError {
  message: String,
  source: Option<Box<dyn Error>>,
}

pub enum Functions {
  FileSearchFunction(FileSearchFunction),
  GrepFunction(GrepFunction),
  ReadFileLinesFunction(ReadFileLinesFunction),
  ModifyFileFunction(ModifyFileFunction),
  CreateFileFunction(CreateFileFunction),
}

impl FunctionCallError {
  pub fn new(message: &str) -> Self {
    trace_dbg!("FunctionCallError: {}", message);
    FunctionCallError { message: message.to_string(), source: None }
  }
}

// Implement the Display trait for your custom error type.
impl fmt::Display for FunctionCallError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "FunctionCallError: {}", self.message)
  }
}

// Implement the Error trait for your custom error type.
impl Error for FunctionCallError {
  fn description(&self) -> &str {
    &self.message
  }

  fn source(&self) -> Option<&(dyn Error + 'static)> {
    self.source.as_ref().map(|e| e.as_ref())
  }
}

impl From<grep::regex::Error> for FunctionCallError {
  fn from(error: grep::regex::Error) -> Self {
    FunctionCallError { message: format!("Grep Regex Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<serde_json::Error> for FunctionCallError {
  fn from(error: serde_json::Error) -> Self {
    FunctionCallError { message: format!("Serde JSON Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<io::Error> for FunctionCallError {
  fn from(error: io::Error) -> Self {
    FunctionCallError { message: format!("IO Error: {}", error), source: Some(Box::new(error)) }
  }
}

impl From<String> for FunctionCallError {
  fn from(message: String) -> Self {
    FunctionCallError { message, source: None }
  }
}

pub trait FunctionCall {
  fn init() -> Self;
  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Result<Option<String>, FunctionCallError>;
  fn command_definition(&self) -> Command;
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

pub fn count_tokens(text: &str) -> usize {
  let bpe = tiktoken_rs::cl100k_base().unwrap();
  bpe.encode_with_special_tokens(text).len()
}

pub fn define_commands() -> Vec<Command> {
  let mut commands: Vec<Command> = Vec::new();
  let file_search_function = FileSearchFunction::init();
  commands.push(file_search_function.command_definition());
  commands
  // let command = Command {
  //   name: "create_file".to_string(),
  //   description: Some("create a text file".to_string()),
  //   parameters: Some(CommandParameters {
  //     param_type: "object".to_string(),
  //     required: vec![],
  //     properties: HashMap::from([
  //       (
  //         "path".to_string(),
  //         CommandProperty {
  //           property_type: "string".to_string(),
  //           description: Some("path to create file. all file paths must start with ./".to_string()),
  //           enum_values: None,
  //         },
  //       ),
  //       (
  //         "text".to_string(),
  //         CommandProperty {
  //           property_type: "string".to_string(),
  //           description: Some("text to enter into file".to_string()),
  //           enum_values: None,
  //         },
  //       ),
  //     ]),
  //   }),
  // };
  // commands.push(command);
  // let command = Command {
  //   name: "file_search".to_string(),
  //   description: Some(
  //     "search accessible file paths. file_search without arguments returns all accessible file paths. results include file line count".to_string(),
  //   ),
  //   parameters: Some(CommandParameters {
  //     param_type: "object".to_string(),
  //     required: vec![],
  //     properties: HashMap::from([(
  //       "search_term".to_string(),
  //       CommandProperty {
  //         property_type: "string".to_string(),
  //         description: Some(
  //           "fuzzy search for files by name or path. search results contain a match score and line count.".to_string(),
  //         ),
  //         enum_values: None,
  //       },
  //     )]),
  //   }),
  // };
  // commands.push(command);
  // let command = Command {
  //   name: "read_lines".to_string(),
  //   description: Some("read lines from an accessible file path".to_string()),
  //   parameters: Some(CommandParameters {
  //     param_type: "object".to_string(),
  //     required: vec!["path".to_string()],
  //     properties: HashMap::from([
  //       (
  //         "path".to_string(),
  //         CommandProperty {
  //           property_type: "string".to_string(),
  //           description: Some("path to file".to_string()),
  //           enum_values: None,
  //         },
  //       ),
  //       (
  //         "start_line".to_string(),
  //         CommandProperty {
  //           property_type: "number".to_string(),
  //           description: Some("line to start read - omit to read file from beginning".to_string()),
  //           enum_values: None,
  //         },
  //       ),
  //       (
  //         "end_line".to_string(),
  //         CommandProperty {
  //           property_type: "number".to_string(),
  //           description: Some("line to end read - omit to read file to EOF".to_string()),
  //           enum_values: None,
  //         },
  //       ),
  //     ]),
  //   }),
  // };
  // commands.push(command);
  // let command = Command {
  //   name: "modify_file".to_string(),
  //   description: Some("modify a file by adding, removing, or replacing lines of text".to_string()),
  //   parameters: Some(CommandParameters {
  //     param_type: "object".to_string(),
  //     required: vec!["path".to_string(), "start_line".to_string()],
  //     properties: HashMap::from([
  //       (
  //         "path".to_string(),
  //         CommandProperty {
  //           property_type: "string".to_string(),
  //           description: Some("path to file".to_string()),
  //           enum_values: None,
  //         },
  //       ),
  //       (
  //         "start_line".to_string(),
  //         CommandProperty {
  //           property_type: "number".to_string(),
  //           description: Some("line number to begin add and remove".to_string()),
  //           enum_values: None,
  //         },
  //       ),
  //       (
  //         "end_line".to_string(),
  //         CommandProperty {
  //           property_type: "number".to_string(),
  //           description: Some("last line to remove, starting at start_line. Omit end_line to insert text at starting line without removal".to_string()),
  //           enum_values: None,
  //         },
  //       ),
  //       (
  //         "insert_text".to_string(),
  //         CommandProperty {
  //           property_type: "string".to_string(),
  //           description: Some("text to insert at start_line, after optional removal".to_string()),
  //           enum_values: None,
  //         },
  //       ),
  //     ]),
  //   }),
  // };
  // commands.push(command);
  // // let command = Command {
  // //     name: "cargo check".to_string(),
  // //     description: Some("run cargo check to discover any compilation errors".to_string()),
  // //     parameters: None,
  // // };
  // // commands.push(command);
  // commands
}

pub fn handle_chat_response_function_call(
  tx: UnboundedSender<Action>,
  fn_name: String,
  fn_args: String,
  session_config: SessionConfig,
) {
  tokio::spawn(async move {
    match {
      let fn_name = fn_name.clone();
      let fn_args = fn_args;
      async move {
        let function_args: Result<HashMap<String, serde_json::Value>, serde_json::Error> =
          serde_json::from_str(fn_args.as_str());
        match function_args {
          Ok(function_args) => match fn_name.as_str() {
            "create_file" => CreateFileFunction::init().call(function_args, session_config),
            "grep" => GrepFunction::init().call(function_args, session_config),
            "file_search" => FileSearchFunction::init().call(function_args, session_config),
            "read_lines" => ReadFileLinesFunction::init().call(function_args, session_config),
            "modify_file" => ModifyFileFunction::init().call(function_args, session_config),
            "cargo_check" => CargoCheckFunction::init().call(function_args, session_config),
            _ => Ok(None),
          },
          Err(e) => Err(FunctionCallError::new(
            format!("Failed to parse function arguments:\nfunction:{:?}\nargs:{:?}\nerror:{:?}", fn_name, fn_args, e)
              .as_str(),
          )),
        }
      }
    }
    .await
    {
      Ok(Some(output)) => {
        //self.data.add_message(ChatMessage::FunctionResult(FunctionResult { name: fn_name, response: output }));
        tx.send(Action::AddMessage(ChatMessage::FunctionResult(FunctionResult { name: fn_name, response: output })))
          .unwrap();
      },
      Ok(None) => {},
      Err(e) => {
        // self.data.add_message(ChatMessage::FunctionResult(FunctionResult {
        //   name: fn_name,
        //   response: format!("Error: {:?}", e),
        // }));
        tx.send(Action::AddMessage(ChatMessage::FunctionResult(FunctionResult {
          name: fn_name,
          response: format!("Error: {:?}", e),
        })))
        .unwrap();
      },
    }
    tx.send(Action::RequestChatCompletion()).unwrap();
  });
}
