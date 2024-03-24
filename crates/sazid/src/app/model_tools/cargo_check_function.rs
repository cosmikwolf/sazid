use std::{collections::HashMap, pin::Pin};

use futures_util::Future;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
  errors::ToolCallError,
  tool_call::{ToolCallParams, ToolCallTrait},
  types::{FunctionParameters, FunctionProperties, ToolCall},
};

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct CargoCheckFunction {
  name: String,
  description: String,
  required_properties: Vec<FunctionProperties>,
  optional_properties: Vec<FunctionProperties>,
}

impl ToolCallTrait for CargoCheckFunction {
  fn name(&self) -> &str {
    &self.name
  }

  fn init() -> Self {
    CargoCheckFunction {
      name: "cargo_check".to_string(),
      description: "run cargo check".to_string(),
      required_properties: vec![],
      optional_properties: vec![],
    }
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

  fn function_definition(&self) -> ToolCall {
    let mut properties: HashMap<String, FunctionProperties> = HashMap::new();

    self.required_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.optional_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    ToolCall {
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
