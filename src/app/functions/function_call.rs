use std::collections::HashMap;

use async_openai::types::FunctionCallStream;
use serde_derive::{Deserialize, Serialize};

use crate::app::session_config::SessionConfig;

use super::{errors::FunctionCallError, types::Command};

pub trait FunctionCall {
  fn init() -> Self
  where
    Self: Sized;
  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Result<Option<String>, FunctionCallError>;
  fn command_definition(&self) -> Command;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RenderedFunctionCall {
  pub name: String,
  pub arguments: String,
}

impl From<Box<dyn FunctionCall>> for RenderedFunctionCall {
  fn from(function_call: Box<dyn FunctionCall>) -> Self {
    RenderedFunctionCall { name: function_call.command_definition().name, arguments: "".to_string() }
  }
}

impl From<FunctionCallStream> for RenderedFunctionCall {
  fn from(function_call: FunctionCallStream) -> Self {
    RenderedFunctionCall {
      name: function_call.name.unwrap_or("".to_string()),
      arguments: function_call.arguments.unwrap_or("".to_string()),
    }
  }
}
