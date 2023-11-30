use std::collections::HashMap;

use crate::app::session_config::SessionConfig;

use super::{errors::ToolCallError, types::FunctionCall};

pub trait ToolCallTrait {
  fn init() -> Self
  where
    Self: Sized;
  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Result<Option<String>, ToolCallError>;
  fn function_definition(&self) -> FunctionCall;
}
