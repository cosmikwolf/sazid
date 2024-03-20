use async_openai::{
  config::OpenAIConfig,
  types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject},
  Client,
};

use crate::app::types::*;

use super::model_tools::types::ToolCall;

#[derive(Clone)]
pub struct GPTConnector {
  pub settings: GPTSettings,
  pub include_functions: bool,
  pub client: Client<OpenAIConfig>,
  pub model: Model,
}

pub fn create_chat_completion_tool_args(
  commands: Vec<ToolCall>,
) -> Vec<ChatCompletionTool> {
  commands
    .iter()
    .map(|command| ChatCompletionTool {
      r#type: ChatCompletionToolType::Function,
      function: FunctionObject {
        name: command.name.clone(),
        description: command.description.clone(),
        parameters: command
          .parameters
          .as_ref()
          .map(|parameters| serde_json::to_value(parameters).unwrap()),
      },
    })
    .collect()
}

pub fn create_chat_completion_function_args(
  commands: Vec<ToolCall>,
) -> Vec<FunctionObject> {
  let mut chat_completion_functions: Vec<FunctionObject> = Vec::new();
  for command in commands {
    let chat_completion_function = FunctionObject {
      name: command.name,
      description: command.description,
      parameters: command
        .parameters
        .as_ref()
        .map(|parameters| serde_json::to_value(parameters).unwrap()),
    };
    chat_completion_functions.push(chat_completion_function);
  }
  chat_completion_functions
}

#[cfg(test)]
mod test {
  use std::path::PathBuf;

  use crate::app::model_tools::{
    cargo_check_function::cargo_check, file_search_function::file_search,
  };

  #[test]
  fn test_list_dir() {
    let dir_contents =
      file_search(1024, vec![PathBuf::from("src".to_string())], None);
    assert!(dir_contents.is_ok());
  }

  // #[test]
  // fn test_read_file_lines() {
  //   let file_contents =
  //     super::read_file_lines("./src/gpt_commands.rs", Some(0), Some(10), 1024, vec![PathBuf::from("src".to_string())]);
  //   assert!(file_contents.is_ok());
  // }

  #[test]
  fn test_replace_lines() {
    // let replace_result = super::replace_lines("./src/gpt_commands.rs", 0, 10, "test");
    // assert!(replace_result.is_ok());
  }

  #[test]
  fn test_cargo_check() {
    let cargo_check_result = cargo_check();
    assert!(cargo_check_result.is_ok());
  }
}
