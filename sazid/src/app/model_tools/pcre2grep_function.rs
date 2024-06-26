use crate::app::model_tools::tool_call::ToolCallTrait;
use std::{path::PathBuf, pin::Pin};

use super::{
  argument_validation::{
    validate_and_extract_paths_from_argument,
    validate_and_extract_string_argument,
  },
  errors::ToolCallError,
  tool_call::ToolCallParams,
  types::{FunctionProperty, PropertyType},
};

use futures_util::Future;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pcre2GrepFunction {
  pub name: String,
  pub description: String,
  pub properties: Vec<FunctionProperty>,
}

impl Default for Pcre2GrepFunction {
  fn default() -> Self {
    Pcre2GrepFunction {
      name: "pcre2grep".to_string(),
      description: "an implementation of grep".to_string(),
      properties: vec![
        // CommandProperty {
        //   name: "options".to_string(),
        //   required: true,
        //   property_type: Box::new(PropertyType::String)(),
        //   description: Some(format!(
        //     "pcre2grep arguments, space separated. valid options: {}",
        //     clap_args_to_json::<Args>()
        //   )),
        //   enum_values: None,
        // },
        FunctionProperty {
          name: "pattern".to_string(),
          required: true,
          property_type: PropertyType::String,
          description: Some("a regular expression pattern to match against file contents".to_string()),
          enum_values: None,
        },
        FunctionProperty {
          name: "paths".to_string(),
          required: true,
          property_type: PropertyType::String,
          description: Some(
            "a list of comma separated paths to walk for files which the pattern will be matched against".to_string(),
          ),
          enum_values: None,
        },
      ],
    }
  }
}

pub fn execute_pcre2grep(
  // options: Option<Vec<String>>,
  pattern: String,
  paths: Vec<PathBuf>,
) -> Result<Option<String>, ToolCallError> {
  let output = std::process::Command::new("pcre2grep")
    // .args({
    //   if let Some(options) = options {
    //     options
    //   } else {
    //     vec![]
    //   }
    // })
    .arg(pattern)
    .args(paths)
    .output()
    .map_err(|e| ToolCallError::new(e.to_string().as_str()))?;

  if !output.status.success() {
    return Ok(Some(
      ToolCallError::new(output.status.code().unwrap().to_string().as_str())
        .to_string(),
    ));
  }

  Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}

impl ToolCallTrait for Pcre2GrepFunction {
  fn init() -> Self {
    Pcre2GrepFunction::default()
  }
  fn name(&self) -> &str {
    &self.name
  }

  fn call(
    &self,
    params: ToolCallParams,
  ) -> Pin<
    Box<
      dyn Future<Output = Result<Option<String>, ToolCallError>>
        + Send
        + 'static,
    >,
  > {
    let paths = validate_and_extract_paths_from_argument(
      &params.function_args,
      params.session_config,
      true,
      None,
    )
    .expect("error parsing paths")
    .expect("paths are required");

    let pattern = validate_and_extract_string_argument(
      &params.function_args,
      "pattern",
      true,
    )
    .expect("error parsing pattern")
    .expect("pattern is required");

    Box::pin(async move { execute_pcre2grep(pattern, paths) })
  }

  fn parameters(&self) -> Vec<FunctionProperty> {
    self.properties.clone()
  }

  fn description(&self) -> String {
    self.description.clone()
  }
}
