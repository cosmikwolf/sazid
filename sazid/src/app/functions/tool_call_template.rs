use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::app::session_config::SessionConfig;

use super::argument_validation::*;
use super::errors::ToolCallError;
use super::tool_call::ToolCallTrait;
use super::types::*;

/// The command definition structure with metadata for serialization.
// the struct name TemplatedFunction should be renamed appropriately
#[derive(Serialize, Deserialize)]
struct TemplatedFunction {
  pub name: String,
  pub description: String,
  pub properties: Vec<FunctionProperties>,
}

// Implementation of the `ModelFunction` trait for the `SedCommand` struct.
impl ToolCallTrait for TemplatedFunction {
  // The init function is where the properties of the function are defined.
  // These are the properties that are provided to GPT as arguments to the function call
  // Each property should be designated as required or not based on the required boolean
  // Each property also has a property_type, which correlates to the JSON data type that is accepted by the function call
  fn init() -> Self {
    TemplatedFunction {
      name: "function_name".to_string(),
      description: "function description".to_string(),
      properties: vec![
        FunctionProperties {
          name: "required_property".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some("required property description".to_string()),
          enum_values: None,
        },
        FunctionProperties {
          name: "optional_property".to_string(),
          required: false,
          property_type: "string".to_string(),
          description: Some("required property description".to_string()),
          enum_values: None,
        },
      ],
    }
  }

  // This is the code that is executed when the function is called.
  // Its job is to take the function_args, validate them using the functions defined in src/functions/argument_validation.rs
  // It should also handle
  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Result<Option<String>, ToolCallError> {
    // Begin Example Call Code
    match validate_and_extract_paths_from_argument(
      &function_args,
      session_config,
      true,
      None,
    ) {
      Ok(Some(paths)) => match validate_and_extract_boolean_argument(
        &function_args,
        "reverse",
        false,
      ) {
        Ok(reverse) => {
          // This command is an abstraction for a CLI command, so it calls std::process::command, any new function should have whatever implementation is necessary to execute the function, and should return a Result<Option<String>, ToolCallError>
          // If the code is too complex, it should be broken out into another function.
          let output = std::process::Command::new("git")
            .arg("apply")
            .arg("--verbose")
            .args(if reverse.unwrap_or(false) {
              vec!["--reverse"]
            } else {
              vec![]
            })
            .args(paths)
            .output()
            .map_err(|e| ToolCallError::new(e.to_string().as_str()))?;

          if !output.status.success() {
            return Ok(Some(
              ToolCallError::new(&String::from_utf8_lossy(
                output.stderr.as_slice(),
              ))
              .to_string(),
            ));
          }

          Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
        },
        Err(err) => Ok(Some(err.to_string())),
      },
      Ok(None) => Ok(Some("no patch file passed to function".to_string())),
      Err(err) => Ok(Some(err.to_string())),
    }
    // End example call function code
  }

  // this function creates the FunctionCall struct which is used to pass the function to GPT.
  // This code should not change and should be identical for each function call
  fn function_definition(&self) -> FunctionCall {
    let mut properties: HashMap<String, FunctionProperties> = HashMap::new();

    self.properties.iter().filter(|p| p.required).for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.properties.iter().filter(|p| !p.required).for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    FunctionCall {
      name: self.name.clone(),
      description: Some(self.description.clone()),
      parameters: Some(FunctionParameters {
        param_type: "object".to_string(),
        required: self
          .properties
          .clone()
          .into_iter()
          .filter(|p| p.required)
          .map(|p| p.name)
          .collect(),
        properties,
      }),
    }
  }
}
