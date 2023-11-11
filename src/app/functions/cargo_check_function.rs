use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};

use crate::{app::session_config::SessionConfig, trace_dbg};

use super::{
  function_call::ModelFunction,
  types::{Command, CommandParameters, CommandProperty},
  ModelFunctionError,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CargoCheckFunction {
  name: String,
  description: String,
  required_properties: Vec<CommandProperty>,
  optional_properties: Vec<CommandProperty>,
}

impl ModelFunction for CargoCheckFunction {
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
  ) -> Result<Option<String>, ModelFunctionError> {
    cargo_check()
  }

  fn command_definition(&self) -> Command {
    let mut properties: HashMap<String, CommandProperty> = HashMap::new();

    self.required_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.optional_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    Command {
      name: self.name.clone(),
      description: Some(self.description.clone()),
      parameters: Some(CommandParameters {
        param_type: "object".to_string(),
        required: self.required_properties.clone().into_iter().map(|p| p.name).collect(),
        properties,
      }),
    }
  }
}

pub fn cargo_check() -> Result<Option<String>, ModelFunctionError> {
  let mut command = std::process::Command::new("cargo");
  command.arg("check");
  command.arg("--message-format");
  command.arg("json");
  match command.output() {
    Ok(output) => {
      if output.status.success() {
        let cargo_check_json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let output_str = cargo_check_json
          .as_array()
          .unwrap()
          .iter()
          .filter(|m| m["reason"] == "compiler-message")
          .map(|m| m["message"]["rendered"].as_str().unwrap().to_string())
          .collect::<Vec<String>>()
          .join("\n");
        Ok(Some(serde_json::to_string_pretty(&output.stdout).unwrap()))
      } else {
        Ok(Some(format!("cargo check failed: {}", String::from_utf8_lossy(&output.stderr))))
      }
    },
    Err(e) => Ok(Some(format!("cargo check failed: {}", e))),
  }
  //trace_dbg!("{}", String::from_utf8_lossy(&output.stdout));
}
