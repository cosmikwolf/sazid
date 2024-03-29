use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::app::session_config::SessionConfig;

use super::{
  tool_call::ToolCallTrait,
  types::{FunctionCall, FunctionParameters, FunctionProperties},
  ToolCallError,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CargoCheckFunction {
  name: String,
  description: String,
  required_properties: Vec<FunctionProperties>,
  optional_properties: Vec<FunctionProperties>,
}

impl ToolCallTrait for CargoCheckFunction {
  fn init() -> Self {
    CargoCheckFunction {
      name: "cargo_check".to_string(),
      description: "run cargo check --message-format json".to_string(),
      required_properties: vec![],
      optional_properties: vec![],
    }
  }

  fn call(
    &self,
    _function_args: HashMap<String, serde_json::Value>,
    _session_config: SessionConfig,
  ) -> Result<Option<String>, ToolCallError> {
    cargo_check()
  }

  fn function_definition(&self) -> FunctionCall {
    let mut properties: HashMap<String, FunctionProperties> = HashMap::new();

    self.required_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.optional_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    FunctionCall {
      name: self.name.clone(),
      description: Some(self.description.clone()),
      parameters: Some(FunctionParameters {
        param_type: "object".to_string(),
        required: self
          .required_properties
          .clone()
          .into_iter()
          .map(|p| p.name)
          .collect(),
        properties,
      }),
    }
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
          serde_json::from_slice(&output.stdout).unwrap();
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
