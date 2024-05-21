use std::{
  path::PathBuf,
  time::{SystemTime, UNIX_EPOCH},
};

use async_openai::types::{ChatCompletionRequestSystemMessage, Role};
use serde::{Deserialize, Serialize};

use super::{consts::*, types::Model};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkspaceParams {
  pub workspace_path: PathBuf,
  pub language: String,
  pub language_server: String,
  pub doc_path: Option<PathBuf>,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SessionConfig {
  pub prompt: String,
  pub id: String,
  pub session_dir: PathBuf,
  pub disabled_tools: Vec<String>,
  pub tools_enabled: bool,
  pub accessible_paths: Vec<PathBuf>,
  pub workspace: Option<WorkspaceParams>,
  pub model: Model,
  pub retrieval_augmentation_message_count: Option<i64>,
  pub user: String,
  pub include_functions: bool,
  pub stream_response: bool,
  pub function_result_max_tokens: usize,
  pub response_max_tokens: usize,
  pub database_url: String,
}

impl Default for SessionConfig {
  fn default() -> Self {
    SessionConfig {
      prompt: String::new(),
      id: Self::generate_session_id(),
      session_dir: PathBuf::new(),
      disabled_tools: vec![],
      workspace: None,
      tools_enabled: true,
      accessible_paths: vec![],
      model: GPT4_O.clone(),
      retrieval_augmentation_message_count: Some(10),
      user: "sazid_user_1234".to_string(),
      function_result_max_tokens: 8192,
      response_max_tokens: 4095,
      include_functions: true,
      stream_response: true,
      database_url: String::new(),
    }
  }
}

impl SessionConfig {
  pub fn prompt_message(&self) -> ChatCompletionRequestSystemMessage {
    ChatCompletionRequestSystemMessage {
      content: self.prompt.clone(),
      name: Some(self.user.clone()),
      role: Role::System,
    }
  }

  pub fn generate_session_id() -> String {
    // Get the current time since UNIX_EPOCH in seconds.
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards").as_secs();

    // Introduce a delay of 1 second to ensure unique session IDs even if called rapidly.
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Convert the duration to a String and return.
    since_the_epoch.to_string()
  }
}
