use futures_util::Future;
use lsp_types::SymbolKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;

use crate::action::{ChatToolAction, LsiAction};
use crate::app::lsi::query::LsiQuery;

use super::argument_validation::*;
use super::errors::ToolCallError;
use super::tool_call::{ToolCallParams, ToolCallTrait};
use super::types::*;

#[derive(Serialize, Deserialize)]
pub struct LspQuerySymbol {
  pub name: String,
  pub description: String,
  pub properties: Vec<FunctionProperty>,
}

impl ToolCallTrait for LspQuerySymbol {
  fn init() -> Self
  where
    Self: Sized,
  {
    LspQuerySymbol {
            name: "lsp_query".to_string(),
            description: "query symbols in project source code using a language server".to_string(),
            properties: vec![
                FunctionProperty {
                    name: "name_regex".to_string(),
                    required: false,
                    property_type: PropertyType::Pattern,
                    description: Some("include results where the symbol name matches".to_string()),
                    enum_values: None,
                },
                FunctionProperty {
                name: "kind".to_string(),
                required: false,
                property_type: PropertyType::String,
                description: Some("filter results by kind: MODULE NAMESPACE PACKAGE CLASS METHOD PROPERTY FIELD CONSTRUCTOR ENUM INTERFACE FUNCTION VARIABLE CONSTANT STRING NUMBER BOOLEAN ARRAY OBJECT KEY NULL ENUM_MEMBER STRUCT EVENT OPERATOR TYPE_PARAMETER".to_string()),
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
                name: "file_path_regex".to_string(),
                required: false,
                property_type: PropertyType::Pattern,
                description: Some("include results where the file path matches".to_string()),
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
    let workspace_root = params
      .session_config
      .workspace
      .as_ref()
      .expect("workspace not set")
      .workspace_path
      .to_path_buf();

    let name_regex = validate_and_extract_pattern_argument(
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

    let range = validate_and_extract_range_argument(
      &params.function_args,
      "range",
      false,
    )
    .expect("error validating range");

    let file_path_regex = validate_and_extract_pattern_argument(
      &params.function_args,
      "file_name",
      false,
    )
    .expect("error validating file_glob");

    Box::pin(async move {
      params
        .session_config
        .workspace
        .expect("workspace must be initialized before query");

      let kind: Option<SymbolKind> = kind.and_then(|kind| {
        let kind = change_case::pascal_case(&kind);
        SymbolKind::try_from(kind.as_str()).ok()
      });

      let query = LsiQuery {
        name_regex: name_regex.map(|p| p.to_string()),
        kind,
        range,
        workspace_root,
        tool_call_id: params.tool_call_id,
        session_id: params.session_id,
        file_path_regex: file_path_regex.map(|p| p.to_string()),
        diagnostic_severity: None,
        ..Default::default()
      };

      params
        .tx
        .send(ChatToolAction::LsiRequest(Box::new(
          LsiAction::QueryWorkspaceSymbols(query),
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
