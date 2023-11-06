use std::collections::HashMap;

use crate::app::session_config::SessionConfig;

use super::{
  types::{Command, CommandParameters, CommandProperty},
  FunctionCall, FunctionCallError,
};

pub struct CargoCheckFunction {
  name: String,
  description: String,
  required_properties: Vec<CommandProperty>,
  optional_properties: Vec<CommandProperty>,
}

impl FunctionCall for CargoCheckFunction {
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
    _function_args: HashMap<String, serde_json::Value>,
    _session_config: SessionConfig,
  ) -> Result<Option<String>, FunctionCallError> {
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

pub fn cargo_check() -> Result<Option<String>, FunctionCallError> {
  let mut command = std::process::Command::new("cargo");
  command.arg("check");
  let output = command.output()?;
  println!("{}", String::from_utf8_lossy(&output.stdout));
  Ok(None)
}
