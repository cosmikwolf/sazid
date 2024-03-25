use std::{collections::HashMap, pin::Pin};

use futures_util::Future;
use serde::{Deserialize, Serialize};

use super::{
  errors::ToolCallError,
  tool_call::{ToolCallParams, ToolCallTrait},
  types::FunctionProperty,
};

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct CargoCheckFunction {
  name: String,
  description: String,
  properties: Vec<FunctionProperty>,
}

impl ToolCallTrait for CargoCheckFunction {
  fn name(&self) -> &str {
    &self.name
  }

  fn init() -> Self {
    CargoCheckFunction {
      name: "cargo_check".to_string(),
      description: "run cargo check".to_string(),
      properties: vec![],
    }
  }

  fn properties(&self) -> Vec<FunctionProperty> {
    self.properties.clone()
  }

  fn description(&self) -> String {
    self.description.clone()
  }

  fn call(
    &self,
    _params: ToolCallParams,
  ) -> Pin<
    Box<
      dyn Future<Output = Result<Option<String>, ToolCallError>>
        + Send
        + 'static,
    >,
  > {
    Box::pin(async move { cargo_check() })
  }
}

pub fn cargo_check() -> Result<Option<String>, ToolCallError> {
  let mut command = std::process::Command::new("cargo");
  command.arg("check");
  command.arg("--message-format");
  command.arg("json");
  match command.output() {
    Ok(output) => {
      if output.status.success() {
        let cargo_check_json: serde_json::Value =
          match serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            Ok(value) => value,
            Err(e) => {
              return Ok(Some(format!("cargo check failed: {}", e)));
            },
          };
        let _output_str = cargo_check_json
          .as_array()
          .unwrap()
          .iter()
          .filter(|m| m["reason"] == "compiler-message")
          .map(|m| m["message"]["rendered"].as_str().unwrap().to_string())
          .collect::<Vec<String>>()
          .join("\n");
        Ok(Some(serde_json::to_string_pretty(&output.stdout).unwrap()))
      } else {
        Ok(Some(format!(
          "cargo check failed: {}",
          String::from_utf8_lossy(&output.stderr)
        )))
      }
    },
    Err(e) => Ok(Some(format!("cargo check failed: {}", e))),
  }
  //trace_dbg!("{}", String::from_utf8_lossy(&output.stdout));
}
