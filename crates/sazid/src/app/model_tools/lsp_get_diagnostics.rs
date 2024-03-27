use futures_util::Future;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;

use crate::action::{ChatToolAction, LsiAction};
use crate::app::lsi::query::{DiagnosticIncludeFlags, LsiQuery};

use super::errors::ToolCallError;
use super::tool_call::{ToolCallParams, ToolCallTrait};
use super::types::*;

#[derive(Serialize, Deserialize)]
pub struct LspGetDiagnostics {
  pub name: String,
  pub description: String,
  pub properties: Vec<FunctionProperty>,
}

impl ToolCallTrait for LspGetDiagnostics {
  fn init() -> Self
  where
    Self: Sized,
  {
    LspGetDiagnostics {
        name: "lsp_diagnostics".to_string(),
        description: "get language server diagnostic information".to_string(),
        properties: vec![
            FunctionProperty {
                name: "errors".to_string(),
                required: true,
                property_type: PropertyType::Boolean,
                description: Some("include errors in the diagnostic report".to_string()),
                enum_values: None,
            },
            FunctionProperty {
                name: "warnings".to_string(),
                required: true,
                property_type: PropertyType::Boolean,
                description: Some("include warnings in the diagnostic report".to_string()),
                enum_values: None,
            },
            FunctionProperty {
                name: "information".to_string(),
                required: true,
                property_type: PropertyType::Boolean,
                description: Some("include diagnostics classified as information in the diagnostic report".to_string()),
                enum_values: None,
            },
            FunctionProperty {
                name: "hints".to_string(),
                required: true,
                property_type: PropertyType::Boolean,
                description: Some("include diagnostics classified as hints in the diagnostic report".to_string()),
                enum_values: None,
            },
            FunctionProperty {
                name: "no_severity".to_string(),
                required: true,
                property_type: PropertyType::Boolean,
                description: Some("include diagnostics with no severity classification in the diagnostic report".to_string()),
                enum_values: None,
            },
            FunctionProperty {
                name: "file_path_regex".to_string(),
                required: false,
                property_type: PropertyType::Pattern,
                description: Some("include results where the file path matches".to_string()),
                enum_values: None,
            },
            FunctionProperty {
                name: "range".to_string(),
                required: false,
                property_type: PropertyType::String,
                description: Some("filter results by byte range in source file, in the format of start_line,start_char,end_line,end_char. this has no effect if file_path_regex is not specified".to_string()),
                enum_values: None,
            },
        ],
      }
  }

  fn name(&self) -> &str {
    &self.name
  }
  fn properties(&self) -> Vec<FunctionProperty> {
    self.properties.clone()
  }

  fn description(&self) -> String {
    self.description.clone()
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
    let validated_arguments =
      validate_arguments(params.function_args, &self.properties, None)
        .expect("error validating arguments");

    let file_path_regex =
      get_validated_argument(&validated_arguments, "file_path_regex");

    let include_errors =
      get_validated_argument(&validated_arguments, "include_errors");

    let include_warnings =
      get_validated_argument(&validated_arguments, "include_warnings");

    let include_information =
      get_validated_argument(&validated_arguments, "include_information");

    let include_hints =
      get_validated_argument(&validated_arguments, "include_hints");

    let include_no_severity =
      get_validated_argument(&validated_arguments, "include_no_severity");

    let range = get_validated_argument(&validated_arguments, "range");

    Box::pin(async move {
      // Begin Example Call Code
      // This command is an abstraction for a CLI command, so it calls std::process::command, any new function should have whatever implementation is necessary to execute the function, and should return a Result<Option<String>, ToolCallError>
      // If the code is too complex, it should be broken out into another function.

      let query = LsiQuery {
        range,
        file_path_regex,
        tool_call_id: params.tool_call_id,
        session_id: params.session_id,
        diagnostic_severity: Some(DiagnosticIncludeFlags {
          include_errors,
          include_warnings,
          include_information,
          include_hints,
          include_no_severity,
        }),
        ..Default::default()
      };

      params
        .tx
        .send(ChatToolAction::LsiRequest(Box::new(LsiAction::GetDiagnostics(
          query,
        ))))
        .unwrap();
      // return none, so the tool completes when it receieves a response from the language server
      Ok(None)
    }) // End example call function code
  }

  // this function creates the FunctionCall struct which is used to pass the function to GPT.
  // This code should not change and should be identical for each function call
  fn function_definition(&self) -> ToolCall {
    let mut properties: HashMap<String, FunctionProperty> = HashMap::new();

    self.properties.iter().filter(|p| p.required).for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.properties.iter().filter(|p| !p.required).for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    ToolCall {
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
