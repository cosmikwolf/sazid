use futures_util::Future;
use lsp_types::{Range, SymbolKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;

use crate::action::{ChatToolAction, LsiAction};
use crate::app::lsp::symbol_types::SymbolQuery;

use super::argument_validation::*;
use super::errors::ToolCallError;
use super::tool_call::{ToolCallParams, ToolCallTrait};
use super::types::*;

/// The command definition structure with metadata for serialization.
// the struct name LspTool should be renamed appropriately
#[derive(Serialize, Deserialize)]
pub struct LspTool {
  pub name: String,
  pub description: String,
  pub properties: Vec<FunctionProperty>,
}

pub enum LspToolVariant {
  SymbolQuery,
}
// Implementation of the `ModelFunction` trait for the `SedCommand` struct.
impl ToolCallTrait for LspTool {
  // This is the code that is executed when the function is called.
  // Its job is to take the function_args, validate them using the functions defined in src/functions/argument_validation.rs
  // It should also handle

  fn init() -> Self
  where
    Self: Sized,
  {
    LspTool {
            name: "language_server".to_string(),
            description: "query symbols in project source code using a language server".to_string(),
            properties: vec![
                FunctionProperty {
                    name: "name".to_string(),
                    required: false,
                    property_type: PropertyType::String,
                    description: Some("filter results by name".to_string()),
                    enum_values: None,
                },
                FunctionProperty {
                name: "kind".to_string(),
                required: false,
                property_type: PropertyType::String,
                description: Some("filter results by kind: FILE MODULE NAMESPACE PACKAGE CLASS METHOD PROPERTY FIELD CONSTRUCTOR ENUM INTERFACE FUNCTION VARIABLE CONSTANT STRING NUMBER BOOLEAN ARRAY OBJECT KEY NULL ENUM_MEMBER STRUCT EVENT OPERATOR TYPE_PARAMETER".to_string()),
                    enum_values: None,
                },
                FunctionProperty {
                name: "range".to_string(),
                required: false,
                property_type: PropertyType::String,
                description: Some("filter results by byte range in source file, in the format of start_line,start_char,end_line,end_char".to_string()),
                enum_values: None,
                },
                FunctionProperty {
                name: "file_name".to_string(),
                required: false,
                property_type: PropertyType::String,
                description: Some("filter results source file, based on file name".to_string()),
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
    let name = validate_and_extract_string_argument(
      &params.function_args,
      "name",
      false,
    )
    .expect("error validating name");

    let kind = validate_and_extract_string_argument(
      &params.function_args,
      "kind",
      false,
    )
    .expect("error validating kind");

    let range = validate_and_extract_string_argument(
      &params.function_args,
      "range",
      false,
    )
    .expect("error validating range");

    let file_name = validate_and_extract_string_argument(
      &params.function_args,
      "file_name",
      false,
    )
    .expect("error validating file_glob");

    let workspace = params.session_config.workspace.expect("workspace not set");

    Box::pin(async move {
      // Begin Example Call Code
      // This command is an abstraction for a CLI command, so it calls std::process::command, any new function should have whatever implementation is necessary to execute the function, and should return a Result<Option<String>, ToolCallError>
      // If the code is too complex, it should be broken out into another function.

      let kind: Option<SymbolKind> = kind.and_then(|kind| {
        let kind = change_case::pascal_case(&kind);
        SymbolKind::try_from(kind.as_str()).ok()
      });

      let range: Option<Range> = match range {
        Some(range) => {
          let range: Vec<&str> = range.split(',').collect();
          if range.len() != 4 {
            return Err(ToolCallError::new("Invalid range"));
          }
          let start_line = range[0].parse::<u32>();
          let start_char = range[1].parse::<u32>();
          let end_line = range[2].parse::<u32>();
          let end_char = range[3].parse::<u32>();

          match (start_line, start_char, end_line, end_char) {
            (Ok(sl), Ok(sc), Ok(el), Ok(ec)) => Some(Range {
              start: lsp_types::Position { line: sl, character: sc },
              end: lsp_types::Position { line: el, character: ec },
            }),
            _ => return Err(ToolCallError::new("Failed to parse range")),
          }
        },
        None => None,
      };

      let query = SymbolQuery { name, kind, range, file_name };

      params
        .tx
        .send(ChatToolAction::LsiRequest(Box::new(
          LsiAction::QueryWorkspaceSymbols(
            query,
            workspace.workspace_path,
            params.session_id,
            params.tool_call_id,
          ),
        )))
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
