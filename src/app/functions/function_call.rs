use std::collections::HashMap;

use crate::app::session_config::SessionConfig;

use super::{errors::ModelFunctionError, types::Command};

pub trait ModelFunction {
  fn init() -> Self
  where
    Self: Sized;
  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Result<Option<String>, ModelFunctionError>;
  fn command_definition(&self) -> Command;
}
